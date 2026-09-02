import { analyzeMatrix, invertMatrix, solveLeastSquares } from "./adapter";
import { analyzeTransferFunction, rootLocus, type TransferFunction } from "./classicalAnalysis";
import { solveDiscreteLqr } from "./optimalEstimation";
import type { DenseMatrix, NumericalTolerance } from "./contracts";

export function identifyMimoStaticGain(
  inputs: readonly (readonly number[])[],
  outputs: readonly (readonly number[])[],
  tolerance: NumericalTolerance
): any {
  if (inputs.length < 2 || inputs.length !== outputs.length || inputs[0]?.length === 0 || outputs[0]?.length === 0
      || inputs.some((row) => row.length !== inputs[0].length)
      || outputs.some((row) => row.length !== outputs[0].length)) {
    return { status: "rejected", reason: "inconsistent-dimensions" };
  }
  const inputAnalysis = analyzeMatrix(inputs, tolerance);
  if (inputAnalysis.status !== "qualified" || inputAnalysis.rank < inputs[0].length
      || inputAnalysis.conditionNumber > tolerance.conditionLimit) {
    return { status: "rejected", reason: "poor-excitation" };
  }
  const gainMatrix: number[][] = [];
  let residualSquared = 0;
  for (let output = 0; output < outputs[0].length; output += 1) {
    const fit = solveLeastSquares(inputs, outputs.map((row) => row[output]), tolerance);
    if (fit.status !== "qualified") return { status: "rejected", reason: "poor-excitation" };
    gainMatrix.push([...fit.solution]);
    residualSquared += fit.residualNorm ** 2;
  }
  return { status: "qualified", gainMatrix, inputRank: inputAnalysis.rank,
    inputConditionNumber: inputAnalysis.conditionNumber, residualNorm: Math.sqrt(residualSquared) };
}

export function relativeGainArray(gainMatrix: DenseMatrix, tolerance: NumericalTolerance): any {
  const inverse = invertMatrix(gainMatrix, tolerance);
  if (inverse.status !== "qualified") return { status: "rejected", reason: "singular-or-ill-conditioned" };
  const inverseTranspose = transpose(inverse.inverse);
  return { status: "qualified", matrix: gainMatrix.map((row, rowIndex) =>
    row.map((value, column) => value * inverseTranspose[rowIndex][column])) };
}

export function staticDecoupler(gainMatrix: DenseMatrix, tolerance: NumericalTolerance): any {
  const analysis = analyzeMatrix(gainMatrix, tolerance);
  const inverse = invertMatrix(gainMatrix, tolerance);
  if (analysis.status !== "qualified" || inverse.status !== "qualified") {
    return { status: "rejected", reason: "singular-or-ill-conditioned" };
  }
  return { status: "qualified", matrix: inverse.inverse, conditionNumber: analysis.conditionNumber,
    singularValues: analysis.singularValues };
}

export function analyzeRobustSisoSet(problem: {
  plants: readonly TransferFunction[];
  controller: TransferFunction;
  frequenciesRadPerSecond: readonly number[];
  sensitivityWeight: readonly number[];
  complementarySensitivityWeight: readonly number[];
  performanceLimit: number;
}): any {
  if (problem.plants.length === 0 || problem.frequenciesRadPerSecond.length === 0
      || problem.sensitivityWeight.length !== problem.frequenciesRadPerSecond.length
      || problem.complementarySensitivityWeight.length !== problem.frequenciesRadPerSecond.length
      || !Number.isFinite(problem.performanceLimit) || problem.performanceLimit <= 0) {
    return { status: "rejected", reason: "invalid-uncertainty-set" };
  }
  const cases: any[] = [];
  for (const plant of problem.plants) {
    const loop: TransferFunction = {
      numerator: convolve(plant.numerator, problem.controller.numerator),
      denominator: convolve(plant.denominator, problem.controller.denominator),
      delaySeconds: plant.delaySeconds + problem.controller.delaySeconds,
    };
    if (loop.delaySeconds !== 0) return { status: "rejected", reason: "delay-stability-unavailable" };
    const analysis = analyzeTransferFunction(loop, problem.frequenciesRadPerSecond);
    const locus = rootLocus(loop, [1]);
    if (analysis.status !== "qualified" || locus.status !== "qualified") {
      return { status: "rejected", reason: "invalid-uncertainty-case" };
    }
    const stable = locus.branches[0].closedLoopPoles.every((pole: any) => pole.real < 0);
    const weightedValues = analysis.bode.map((point: any, index: number) => Math.max(
      problem.sensitivityWeight[index] * Math.hypot(point.sensitivity.real, point.sensitivity.imaginary),
      problem.complementarySensitivityWeight[index]
        * Math.hypot(point.complementarySensitivity.real, point.complementarySensitivity.imaginary)
    ));
    cases.push({ stable, worstWeightedValue: Math.max(...weightedValues), weightedValues,
      closedLoopPoles: locus.branches[0].closedLoopPoles });
  }
  const worstWeightedValue = Math.max(...cases.map((item) => item.worstWeightedValue));
  const robustlyStable = cases.every((item) => item.stable);
  return { status: "qualified", cases, robustlyStable,
    worstWeightedValue, robustPerformance: robustlyStable && worstWeightedValue <= problem.performanceLimit };
}

export function updateSupervisedRls(input: any, tolerance: NumericalTolerance): any {
  const valid = input.parameters.length > 0 && input.parameters.length === input.regressor.length
    && input.covariance.length === input.parameters.length
    && input.covariance.every((row: number[]) => row.length === input.parameters.length)
    && input.parameterMinimum.length === input.parameters.length
    && input.parameterMaximum.length === input.parameters.length
    && input.fallbackParameters.length === input.parameters.length
    && [...input.parameters, ...input.regressor, ...input.covariance.flat(), input.measurement,
      input.forgettingFactor, input.minimumExcitation, input.confidenceLimit].every(Number.isFinite)
    && input.forgettingFactor > 0 && input.forgettingFactor <= 1;
  if (!valid) return { status: "rejected", reason: "invalid-estimator-state" };
  if (input.reset) {
    const covariance = identity(input.parameters.length);
    return result("reset", input.fallbackParameters, covariance, input, "explicit-reset");
  }
  const excitation = Math.hypot(...input.regressor);
  const confidence = Math.max(...input.covariance.map((row: number[], index: number) => row[index]));
  if (input.quality !== "good" || excitation < input.minimumExcitation || confidence > input.confidenceLimit) {
    const reason = input.quality !== "good" ? "bad-quality"
      : excitation < input.minimumExcitation ? "poor-excitation" : "low-confidence";
    return result("frozen", input.parameters, input.covariance, input, reason);
  }
  const pPhi = multiplyVector(input.covariance, input.regressor);
  const denominator = input.forgettingFactor + dot(input.regressor, pPhi);
  if (!Number.isFinite(denominator) || denominator <= tolerance.absolute) {
    return result("frozen", input.parameters, input.covariance, input, "singular-update");
  }
  const gain = pPhi.map((value) => value / denominator);
  const innovation = input.measurement - dot(input.regressor, input.parameters);
  const parameters = input.parameters.map((value: number, index: number) => Math.max(
    input.parameterMinimum[index], Math.min(input.parameterMaximum[index], value + gain[index] * innovation)
  ));
  const covariance = input.covariance.map((row: number[], rowIndex: number) => row.map((value, column) =>
    (value - gain[rowIndex] * dot(input.regressor, input.covariance.map((candidate: number[]) => candidate[column])))
      / input.forgettingFactor));
  return { ...result("updated", parameters, covariance, input, "qualified-update"), gain, innovation };
}

export function designCentralizedMimoLqr(problem: {
  a: DenseMatrix; b: DenseMatrix; q: DenseMatrix; r: DenseMatrix;
}, tolerance: NumericalTolerance): any {
  if ((problem.b[0]?.length ?? 0) < 2) return { status: "rejected", reason: "mimo-input-required" };
  const design = solveDiscreteLqr(problem.a, problem.b, problem.q, problem.r, tolerance);
  return design.status === "qualified" ? { ...design, design: "centralized-discrete-lqr" } : design;
}

function result(status: string, parameters: readonly number[], covariance: DenseMatrix, input: any, reason: string): any {
  const nextInput = { ...input, parameters: [...parameters], covariance: covariance.map((row) => [...row]), reset: false };
  return { status, reason, parameters: [...parameters], covariance: covariance.map((row) => [...row]), nextInput };
}
function identity(size: number): number[][] {
  return Array.from({ length: size }, (_, row) => Array.from({ length: size }, (_, column) => row === column ? 1 : 0));
}
function transpose(matrix: DenseMatrix): number[][] {
  return Array.from({ length: matrix[0].length }, (_, column) => matrix.map((row) => row[column]));
}
function multiplyVector(matrix: DenseMatrix, vector: readonly number[]): number[] {
  return matrix.map((row) => dot(row, vector));
}
function dot(left: readonly number[], right: readonly number[]): number {
  return left.reduce((sum, value, index) => sum + value * right[index], 0);
}
function convolve(left: readonly number[], right: readonly number[]): number[] {
  const result = Array(left.length + right.length - 1).fill(0) as number[];
  left.forEach((leftValue, leftIndex) => right.forEach((rightValue, rightIndex) => {
    result[leftIndex + rightIndex] += leftValue * rightValue;
  }));
  return result;
}
