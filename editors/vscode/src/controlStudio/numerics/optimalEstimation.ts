import { eigenvalues, invertMatrix } from "./adapter";
import type { DenseMatrix, NumericalTolerance } from "./contracts";

export function discreteKalmanStep(problem: any, tolerance: NumericalTolerance): any {
  const predictedState = addVector(multiplyVector(problem.a, problem.state), multiplyVector(problem.b, problem.input));
  const predictedCovariance = add(
    multiply(multiply(problem.a, problem.covariance), transpose(problem.a)),
    problem.processNoise
  );
  const innovation = subtractVector(problem.measurement, multiplyVector(problem.c, predictedState));
  const innovationCovariance = add(
    multiply(multiply(problem.c, predictedCovariance), transpose(problem.c)),
    problem.measurementNoise
  );
  const inverse = invertMatrix(innovationCovariance, tolerance);
  if (inverse.status !== "qualified") return { status: "rejected", reason: "singular-innovation" };
  const gain = multiply(multiply(predictedCovariance, transpose(problem.c)), inverse.inverse);
  const state = addVector(predictedState, multiplyVector(gain, innovation));
  const covariance = multiply(subtract(identity(problem.a.length), multiply(gain, problem.c)), predictedCovariance);
  if (![...state, ...covariance.flat()].every(Number.isFinite)) return { status: "rejected", reason: "nonfinite-input" };
  return { status: "qualified", state, covariance, gain, innovation, predictedState, predictedCovariance };
}

export function solveDiscreteLqr(
  a: DenseMatrix,
  b: DenseMatrix,
  q: DenseMatrix,
  r: DenseMatrix,
  tolerance: NumericalTolerance
): any {
  let riccati = copy(q);
  let converged = false;
  for (let iteration = 0; iteration < 10_000; iteration += 1) {
    const next = dareUpdate(a, b, q, r, riccati, tolerance);
    if (!next) return { status: "rejected", reason: "singular-control-weight" };
    if (maxDifference(next, riccati) <= tolerance.absolute + tolerance.relative * maxAbs(next)) {
      riccati = next;
      converged = true;
      break;
    }
    riccati = next;
  }
  if (!converged) return { status: "rejected", reason: "riccati-not-converged" };
  const controlWeight = add(r, multiply(multiply(transpose(b), riccati), b));
  const inverse = invertMatrix(controlWeight, tolerance);
  if (inverse.status !== "qualified") return { status: "rejected", reason: "singular-control-weight" };
  const gain = multiply(multiply(multiply(inverse.inverse, transpose(b)), riccati), a);
  const residual = subtract(riccati, dareUpdate(a, b, q, r, riccati, tolerance)!);
  const closedLoop = subtract(a, multiply(b, gain));
  const poles = eigenvalues(closedLoop, tolerance);
  if (poles.status !== "qualified" || poles.values.some((pole) => Math.hypot(pole.real, pole.imaginary) >= 1)) {
    return { status: "rejected", reason: "nonstabilising-solution" };
  }
  return { status: "qualified", gain, riccati, riccatiResidual: maxAbs(residual), closedLoopPoles: poles.values };
}

export function designDiscreteLqg(problem: any, tolerance: NumericalTolerance): any {
  const lqr = solveDiscreteLqr(problem.a, problem.b, problem.q, problem.r, tolerance);
  if (lqr.status !== "qualified") return lqr;
  const kalman = steadyStateKalman(problem.a, problem.c, problem.processNoise, problem.measurementNoise, tolerance);
  if (kalman.status !== "qualified") return kalman;
  const controllerMatrix = subtract(problem.a, multiply(problem.b, lqr.gain));
  const estimatorMatrix = multiply(subtract(identity(problem.a.length), multiply(kalman.gain, problem.c)), problem.a);
  const controllerPoles = eigenvalues(controllerMatrix, tolerance);
  const estimatorPoles = eigenvalues(estimatorMatrix, tolerance);
  if (controllerPoles.status !== "qualified" || estimatorPoles.status !== "qualified") {
    return { status: "rejected", reason: "solver-failed" };
  }
  return { status: "qualified", lqr, kalman, controllerPoles: controllerPoles.values,
    estimatorPoles: estimatorPoles.values, separationPoles: [...controllerPoles.values, ...estimatorPoles.values] };
}

function steadyStateKalman(a: DenseMatrix, c: DenseMatrix, q: DenseMatrix, r: DenseMatrix, tolerance: NumericalTolerance): any {
  let covariance = copy(q);
  let gain: number[][] = [];
  let residual = Number.POSITIVE_INFINITY;
  for (let iteration = 0; iteration < 10_000; iteration += 1) {
    const predicted = add(multiply(multiply(a, covariance), transpose(a)), q);
    const innovation = add(multiply(multiply(c, predicted), transpose(c)), r);
    const inverse = invertMatrix(innovation, tolerance);
    if (inverse.status !== "qualified") return { status: "rejected", reason: "singular-innovation" };
    gain = multiply(multiply(predicted, transpose(c)), inverse.inverse);
    const next = multiply(subtract(identity(a.length), multiply(gain, c)), predicted);
    residual = maxDifference(next, covariance);
    covariance = next;
    if (residual <= tolerance.absolute + tolerance.relative * maxAbs(next)) {
      return { status: "qualified", gain, covariance, riccatiResidual: residual };
    }
  }
  return { status: "rejected", reason: "riccati-not-converged" };
}

function dareUpdate(a: DenseMatrix, b: DenseMatrix, q: DenseMatrix, r: DenseMatrix,
  p: DenseMatrix, tolerance: NumericalTolerance): number[][] | undefined {
  const inverse = invertMatrix(add(r, multiply(multiply(transpose(b), p), b)), tolerance);
  if (inverse.status !== "qualified") return undefined;
  const at = transpose(a);
  const first = multiply(multiply(at, p), a);
  const correction = multiply(multiply(multiply(multiply(multiply(at, p), b), inverse.inverse), transpose(b)), multiply(p, a));
  return add(subtract(first, correction), q);
}

function identity(size: number): number[][] {
  return Array.from({ length: size }, (_, row) => Array.from({ length: size }, (_, column) => row === column ? 1 : 0));
}
function copy(matrix: DenseMatrix): number[][] { return matrix.map((row) => Array.from(row)); }
function transpose(matrix: DenseMatrix): number[][] {
  return Array.from({ length: matrix[0].length }, (_, column) => matrix.map((row) => row[column]));
}
function add(left: DenseMatrix, right: DenseMatrix): number[][] {
  return left.map((row, rowIndex) => row.map((value, column) => value + right[rowIndex][column]));
}
function subtract(left: DenseMatrix, right: DenseMatrix): number[][] {
  return left.map((row, rowIndex) => row.map((value, column) => value - right[rowIndex][column]));
}
function multiply(left: DenseMatrix, right: DenseMatrix): number[][] {
  return left.map((row) => Array.from({ length: right[0].length }, (_, column) =>
    row.reduce((sum, value, index) => sum + value * right[index][column], 0)));
}
function multiplyVector(matrix: DenseMatrix, vector: readonly number[]): number[] {
  return matrix.map((row) => row.reduce((sum, value, index) => sum + value * vector[index], 0));
}
function addVector(left: readonly number[], right: readonly number[]): number[] {
  return left.map((value, index) => value + right[index]);
}
function subtractVector(left: readonly number[], right: readonly number[]): number[] {
  return left.map((value, index) => value - right[index]);
}
function maxAbs(matrix: DenseMatrix): number { return Math.max(...matrix.flat().map(Math.abs)); }
function maxDifference(left: DenseMatrix, right: DenseMatrix): number {
  return Math.max(...left.flatMap((row, rowIndex) => row.map((value, column) => Math.abs(value - right[rowIndex][column]))));
}
