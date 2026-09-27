import { decodeGeneralModel, type GeneralModel } from "./generalModel";
import { stateSpaceTransferPolynomials } from "./linearTransferModel";
import { makeRationalPolynomial, polynomialGcd, polynomialExactDivide, rationalDivide, rationalNumber } from "./exactPolynomial";
import { rootsOfRealPolynomial } from "./numerics/classicalAnalysis";
export interface ModelPoleView {
  model: GeneralModel; view: "unreduced" | "minimal"; domain: "s" | "z";
  unreduced: { numerator: readonly number[]; denominator: readonly number[] };
  minimal: { numerator: readonly number[]; denominator: readonly number[] };
  cancelledFactor: readonly number[]; hiddenUnstableMode: boolean; warning?: string;
}
/** Reduction is an explicit view. The selected model and full internal modes remain intact. */
export function analyzeModelPoleView(input: unknown, view: unknown): ModelPoleView {
  const model = decodeGeneralModel(input);
  if (!model || !["unreduced", "minimal"].includes(String(view))) throw new Error("Choose a valid model and unreduced or minimal view.");
  if (model.inputs.length !== 1 || model.outputs.length !== 1) throw new Error("Scalar pole-zero cancellation requires a single input/output model.");
  const unreduced = model.kind === "transfer-function" ? { numerator: model.numerator, denominator: model.denominator } : stateSpaceTransferPolynomials(model);
  const numerator = makeRationalPolynomial(unreduced.numerator), denominator = makeRationalPolynomial(unreduced.denominator);
  const gcd = polynomialGcd(numerator, denominator);
  const common = { coefficients: gcd.coefficients.map(c => rationalDivide(c, gcd.coefficients[0])) };
  const reducedNumerator = polynomialExactDivide(numerator, common)!, reducedDenominator = polynomialExactDivide(denominator, common)!;
  const cancelledFactor = common.coefficients.map(rationalNumber), domain = model.kind === "discrete-state-space" ? "z" : "s";
  const poles = rootsOfRealPolynomial(cancelledFactor);
  if (!poles) throw new Error("Cancelled factors exceed the supported pole calculation.");
  const hiddenUnstableMode = poles.some(p => domain === "s" ? p.real >= 0 : Math.hypot(p.real, p.imaginary) >= 1);
  return { model, view: view as ModelPoleView["view"], domain, unreduced,
    minimal: { numerator: reducedNumerator.coefficients.map(rationalNumber), denominator: reducedDenominator.coefficients.map(rationalNumber) }, cancelledFactor, hiddenUnstableMode,
    ...(cancelledFactor.length > 1 ? { warning: `${hiddenUnstableMode ? "An unstable or boundary internal mode is hidden. " : ""}Exact common factors are removed only from the input/output view; the original model remains selected for validation.` }
      : { warning: "No exact common factors in the declared coefficient representation; near cancellations remain unreduced." }) };
}
export function isModelPoleView(value: unknown, selected: GeneralModel): value is ModelPoleView {
  if (!value || typeof value !== "object" || JSON.stringify((value as ModelPoleView).model) !== JSON.stringify(selected)) return false;
  try { return JSON.stringify(analyzeModelPoleView(selected, (value as ModelPoleView).view)) === JSON.stringify(value); } catch { return false; }
}
