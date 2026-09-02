import { eigenvalues, invertMatrix } from "./adapter";
import type { ComplexValue, DenseMatrix, NumericalTolerance } from "./contracts";

export function placeStateFeedbackPoles(
  a: DenseMatrix,
  b: DenseMatrix,
  desiredPoles: readonly number[],
  tolerance: NumericalTolerance
): any {
  const states = a.length;
  if (states === 0 || a.some((row) => row.length !== states) || b.length !== states
      || b.some((row) => row.length !== 1) || desiredPoles.length !== states
      || ![...a.flat(), ...b.flat(), ...desiredPoles].every(Number.isFinite)) {
    return { status: "rejected", reason: "inconsistent-dimensions" };
  }
  const controllability = controllabilityMatrix(a, b);
  const inverse = invertMatrix(controllability, tolerance);
  if (inverse.status !== "qualified") return { status: "rejected", reason: "uncontrollable" };
  const coefficients = polynomialFromRealRoots(desiredPoles);
  let polynomial = scale(identity(states), coefficients[states]);
  let power = identity(states);
  const powers: number[][][] = [power];
  for (let exponent = 1; exponent <= states; exponent += 1) {
    power = multiply(power, a);
    powers.push(power);
  }
  polynomial = powers[states].map((row, rowIndex) => row.map((value, column) =>
    value + polynomial[rowIndex][column]));
  for (let coefficient = 1; coefficient < states; coefficient += 1) {
    const exponent = states - coefficient;
    polynomial = add(polynomial, scale(powers[exponent], coefficients[coefficient]));
  }
  const selector = [inverse.inverse[states - 1]];
  const gain = multiply(selector, polynomial);
  const closedLoop = subtract(a, multiply(b, gain));
  const achieved = eigenvalues(closedLoop, tolerance);
  if (achieved.status !== "qualified") return achieved;
  return {
    status: "qualified",
    gain,
    achievedPoles: achieved.values,
    desiredPoles: desiredPoles.map((real) => ({ real, imaginary: 0 })),
    poleResidual: poleSetResidual(achieved.values, desiredPoles),
    controllabilityMatrix: controllability,
  };
}

export function placeObserverPoles(
  a: DenseMatrix,
  c: DenseMatrix,
  desiredPoles: readonly number[],
  tolerance: NumericalTolerance
): any {
  const dual = placeStateFeedbackPoles(transpose(a), transpose(c), desiredPoles, tolerance);
  if (dual.status !== "qualified") return { status: "rejected", reason: "unobservable" };
  return { ...dual, gain: transpose(dual.gain) };
}

function controllabilityMatrix(a: DenseMatrix, b: DenseMatrix): number[][] {
  const blocks: number[][][] = [copy(b)];
  for (let index = 1; index < a.length; index += 1) blocks.push(multiply(a, blocks[index - 1]));
  return Array.from({ length: a.length }, (_, row) => blocks.flatMap((block) => block[row]));
}

function polynomialFromRealRoots(roots: readonly number[]): number[] {
  return roots.reduce((coefficients, root) => {
    const next = Array(coefficients.length + 1).fill(0) as number[];
    coefficients.forEach((value, index) => {
      next[index] += value;
      next[index + 1] -= value * root;
    });
    return next;
  }, [1]);
}

function poleSetResidual(actual: readonly ComplexValue[], expected: readonly number[]): number {
  const sortedActual = [...actual].sort((left, right) => left.real - right.real || left.imaginary - right.imaginary);
  const sortedExpected = [...expected].sort((left, right) => left - right);
  return Math.max(...sortedActual.map((pole, index) =>
    Math.hypot(pole.real - sortedExpected[index], pole.imaginary)));
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
function scale(matrix: DenseMatrix, factor: number): number[][] {
  return matrix.map((row) => row.map((value) => value * factor));
}
function multiply(left: DenseMatrix, right: DenseMatrix): number[][] {
  return left.map((row) => Array.from({ length: right[0].length }, (_, column) =>
    row.reduce((sum, value, index) => sum + value * right[index][column], 0)));
}
