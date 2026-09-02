import { eigenvalues } from "./adapter";
import { rootsOfRealPolynomial } from "./classicalAnalysis";
import type { DiscreteStateSpaceModel, NumericalTolerance } from "./contracts";

const TOLERANCE: NumericalTolerance = { absolute: 1e-12, relative: 1e-10, conditionLimit: 1e12 };

export function analyzeDiscreteStateSpace(model: DiscreteStateSpaceModel): any {
  if (!Number.isFinite(model.sampleTimeSeconds) || model.sampleTimeSeconds <= 0) {
    return { status: "rejected", reason: "nonfinite-input" };
  }
  const result = eigenvalues(model.a, TOLERANCE);
  if (result.status === "rejected") return result;
  return {
    status: "qualified",
    poles: result.values,
    stable: result.values.every((pole) => Math.hypot(pole.real, pole.imaginary) < 1 - 1e-10),
  };
}

export function sampledRootLocus(
  transfer: { numerator: readonly number[]; denominator: readonly number[]; sampleTimeSeconds: number },
  gains: readonly number[]
): any {
  if (!Number.isFinite(transfer.sampleTimeSeconds) || transfer.sampleTimeSeconds <= 0
      || gains.length === 0 || !gains.every((gain) => Number.isFinite(gain) && gain >= 0)
      || ![...transfer.numerator, ...transfer.denominator].every(Number.isFinite)) {
    return { status: "rejected", reason: "nonfinite-input" };
  }
  const branches = gains.map((gain) => {
    const polynomial = addPolynomials(transfer.denominator, transfer.numerator.map((value) => value * gain));
    const poles = rootsOfRealPolynomial(polynomial);
    if (!poles) throw new Error("The sampled closed-loop polynomial is invalid.");
    return {
      gain,
      poles,
      closedLoopPoles: poles.map((pole) => ({ ...pole })),
      stable: poles.every((pole) => Math.hypot(pole.real, pole.imaginary) < 1 - 1e-10),
    };
  });
  return { status: "qualified", branches };
}

function addPolynomials(left: readonly number[], right: readonly number[]): number[] {
  const length = Math.max(left.length, right.length);
  const paddedLeft = [...Array(length - left.length).fill(0), ...left];
  const paddedRight = [...Array(length - right.length).fill(0), ...right];
  return paddedLeft.map((value, index) => value + paddedRight[index]);
}
