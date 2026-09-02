import {
  EigenvalueDecomposition,
  Matrix,
  SingularValueDecomposition,
  inverse,
} from "ml-matrix";
import type {
  DenseMatrix,
  EigenvalueResult,
  LeastSquaresResult,
  NumericalRejection,
  NumericalTolerance,
} from "./contracts";
import type { ContinuousStateSpaceModel, DiscreteStateSpaceModel } from "./contracts";

export const NUMERICAL_ADAPTER_VERSION = "1";
export const NUMERICAL_DEPENDENCIES = Object.freeze({ "ml-matrix": "6.15.0" });

export function solveLeastSquares(
  coefficients: DenseMatrix,
  observations: readonly number[],
  tolerance: NumericalTolerance
): LeastSquaresResult {
  const rejection = validateRectangular(coefficients, tolerance);
  if (rejection) return rejection;
  const columns = coefficients[0].length;
  if (observations.length !== coefficients.length) return rejected("inconsistent-dimensions");
  if (!observations.every(Number.isFinite)) return rejected("nonfinite-input");
  if (coefficients.length < columns) return rejected("inconsistent-dimensions");

  try {
    const matrix = new Matrix(coefficients.map((row) => Array.from(row)));
    const decomposition = new SingularValueDecomposition(matrix, { autoTranspose: true });
    const singularValues = Array.from(decomposition.diagonal);
    const maximum = Math.max(...singularValues);
    const singularThreshold = Math.max(tolerance.absolute, tolerance.relative * maximum);
    const rank = singularValues.filter((value) => value > singularThreshold).length;
    if (rank < columns) return rejected("singular");
    const minimum = Math.min(...singularValues);
    const conditionNumber = maximum / minimum;
    if (!Number.isFinite(conditionNumber) || conditionNumber > tolerance.conditionLimit) {
      return rejected("ill-conditioned");
    }
    const solved = decomposition.solve(Matrix.columnVector(Array.from(observations)));
    const solution = solved.to1DArray();
    const residualNorm = euclideanNorm(coefficients.map((row, index) =>
      dot(row, solution) - observations[index]));
    if (![...solution, residualNorm].every(Number.isFinite)) return rejected("solver-failed");
    return { status: "qualified", solution, residualNorm, rank, conditionNumber };
  } catch {
    return rejected("solver-failed");
  }
}

export function eigenvalues(matrixData: DenseMatrix, tolerance: NumericalTolerance): EigenvalueResult {
  const rejection = validateRectangular(matrixData, tolerance);
  if (rejection) return rejection;
  if (matrixData.length !== matrixData[0].length) return rejected("matrix-not-square");
  try {
    const decomposition = new EigenvalueDecomposition(
      new Matrix(matrixData.map((row) => Array.from(row)))
    );
    const real = Array.from(decomposition.realEigenvalues);
    const imaginary = Array.from(decomposition.imaginaryEigenvalues);
    const values = real.map((value, index) => ({ real: value, imaginary: imaginary[index] }));
    if (!values.every((value) => Number.isFinite(value.real) && Number.isFinite(value.imaginary))) {
      return rejected("solver-failed");
    }
    return { status: "qualified", values };
  } catch {
    return rejected("solver-failed");
  }
}

export function analyzeMatrix(matrixData: DenseMatrix, tolerance: NumericalTolerance): any {
  const rejection = validateRectangular(matrixData, tolerance);
  if (rejection) return rejection;
  try {
    const decomposition = new SingularValueDecomposition(
      new Matrix(matrixData.map((row) => Array.from(row))), { autoTranspose: true }
    );
    const singularValues = Array.from(decomposition.diagonal);
    const maximum = Math.max(...singularValues);
    const threshold = Math.max(tolerance.absolute, tolerance.relative * maximum);
    const retained = singularValues.filter((value) => value > threshold);
    const rank = retained.length;
    const minimum = Math.min(...singularValues);
    return {
      status: "qualified",
      rank,
      singularValues,
      conditionNumber: minimum <= threshold ? Number.POSITIVE_INFINITY : maximum / minimum,
    };
  } catch {
    return rejected("solver-failed");
  }
}

export function eigenDecomposition(matrixData: DenseMatrix, tolerance: NumericalTolerance): any {
  const rejection = validateRectangular(matrixData, tolerance);
  if (rejection) return rejection;
  if (matrixData.length !== matrixData[0].length) return rejected("matrix-not-square");
  try {
    const decomposition = new EigenvalueDecomposition(new Matrix(copyMatrix(matrixData)));
    const real = Array.from(decomposition.realEigenvalues);
    const imaginary = Array.from(decomposition.imaginaryEigenvalues);
    const vectors = decomposition.eigenvectorMatrix.to2DArray();
    const modes = real.map((value, index) => {
      const imaginaryPart = imaginary[index];
      const vector = vectors.map((row) => {
        if (imaginaryPart > tolerance.absolute) {
          return { real: row[index], imaginary: row[index + 1] };
        }
        if (imaginaryPart < -tolerance.absolute) {
          return { real: row[index - 1], imaginary: -row[index] };
        }
        return { real: row[index], imaginary: 0 };
      });
      return { eigenvalue: { real: value, imaginary: imaginaryPart }, vector };
    });
    return { status: "qualified", values: modes.map((mode) => mode.eigenvalue), modes };
  } catch {
    return rejected("solver-failed");
  }
}

export function discretizeStateSpace(
  model: ContinuousStateSpaceModel,
  sampleTimeSeconds: number,
  method: DiscreteStateSpaceModel["method"]
): { status: "qualified"; model: DiscreteStateSpaceModel }
  | { status: "rejected"; reason: "nonfinite-input" | "inconsistent-dimensions" | "singular" | "solver-failed" } {
  if (!Number.isFinite(sampleTimeSeconds) || sampleTimeSeconds <= 0) {
    return { status: "rejected", reason: "nonfinite-input" };
  }
  const dimensions = stateSpaceDimensions(model);
  if (!dimensions) return { status: "rejected", reason: "inconsistent-dimensions" };
  if (![...model.a.flat(), ...model.b.flat(), ...model.c.flat(), ...model.d.flat()].every(Number.isFinite)) {
    return { status: "rejected", reason: "nonfinite-input" };
  }
  const { states, inputs } = dimensions;
  const identityMatrix = identity(states);
  let a: number[][];
  let b: number[][];
  try {
    if (method === "forward_euler") {
      a = addArrays(identityMatrix, scaleArray(model.a, sampleTimeSeconds));
      b = scaleArray(model.b, sampleTimeSeconds);
    } else if (method === "backward_euler") {
      const resolvent = invertArray(addArrays(identityMatrix, scaleArray(model.a, -sampleTimeSeconds)));
      a = resolvent;
      b = multiplyArrays(resolvent, scaleArray(model.b, sampleTimeSeconds));
    } else if (method === "tustin") {
      const left = addArrays(identityMatrix, scaleArray(model.a, -sampleTimeSeconds / 2));
      const right = addArrays(identityMatrix, scaleArray(model.a, sampleTimeSeconds / 2));
      const resolvent = invertArray(left);
      a = multiplyArrays(resolvent, right);
      b = multiplyArrays(resolvent, scaleArray(model.b, sampleTimeSeconds));
    } else if (method === "exact_zoh") {
      const augmented = Array.from({ length: states + inputs }, () => Array(states + inputs).fill(0) as number[]);
      for (let row = 0; row < states; row += 1) {
        for (let column = 0; column < states; column += 1) augmented[row][column] = model.a[row][column];
        for (let column = 0; column < inputs; column += 1) augmented[row][states + column] = model.b[row][column];
      }
      const exponential = matrixExponential(scaleArray(augmented, sampleTimeSeconds));
      a = exponential.slice(0, states).map((row) => row.slice(0, states));
      b = exponential.slice(0, states).map((row) => row.slice(states, states + inputs));
    } else {
      return { status: "rejected", reason: "solver-failed" };
    }
  } catch {
    return { status: "rejected", reason: "singular" };
  }
  if (![...a.flat(), ...b.flat()].every(Number.isFinite)) {
    return { status: "rejected", reason: "solver-failed" };
  }
  return { status: "qualified", model: { a, b, c: copyMatrix(model.c), d: copyMatrix(model.d),
    sampleTimeSeconds, method } };
}

export function invertMatrix(matrixData: DenseMatrix, tolerance: NumericalTolerance): any {
  const analysis = analyzeMatrix(matrixData, tolerance);
  if (analysis.status !== "qualified") return analysis;
  if (matrixData.length !== matrixData[0].length) return rejected("matrix-not-square");
  if (analysis.rank < matrixData.length) return rejected("singular");
  if (analysis.conditionNumber > tolerance.conditionLimit) return rejected("ill-conditioned");
  try {
    return { status: "qualified", inverse: invertArray(matrixData) };
  } catch {
    return rejected("solver-failed");
  }
}

function validateRectangular(
  matrix: DenseMatrix,
  tolerance: NumericalTolerance
): NumericalRejection | undefined {
  if (!validTolerance(tolerance)) return rejected("nonfinite-input");
  if (matrix.length === 0 || matrix[0]?.length === 0) return rejected("inconsistent-dimensions");
  const columns = matrix[0].length;
  if (!matrix.every((row) => row.length === columns)) return rejected("inconsistent-dimensions");
  if (!matrix.every((row) => row.every(Number.isFinite))) return rejected("nonfinite-input");
  return undefined;
}

function validTolerance(tolerance: NumericalTolerance): boolean {
  return Number.isFinite(tolerance.absolute) && tolerance.absolute > 0
    && Number.isFinite(tolerance.relative) && tolerance.relative > 0
    && Number.isFinite(tolerance.conditionLimit) && tolerance.conditionLimit > 1;
}

function rejected(reason: NumericalRejection["reason"]): NumericalRejection {
  return { status: "rejected", reason };
}

function dot(left: readonly number[], right: readonly number[]): number {
  return left.reduce((sum, value, index) => sum + value * right[index], 0);
}

function euclideanNorm(values: readonly number[]): number {
  return Math.hypot(...values);
}

function stateSpaceDimensions(model: ContinuousStateSpaceModel): { states: number; inputs: number } | undefined {
  const states = model.a.length;
  const inputs = model.b[0]?.length ?? 0;
  const outputs = model.c.length;
  if (states === 0 || inputs === 0 || outputs === 0
      || model.a.some((row) => row.length !== states)
      || model.b.length !== states || model.b.some((row) => row.length !== inputs)
      || model.c.some((row) => row.length !== states)
      || model.d.length !== outputs || model.d.some((row) => row.length !== inputs)) return undefined;
  return { states, inputs };
}

function matrixExponential(matrix: readonly (readonly number[])[]): number[][] {
  const dimension = matrix.length;
  const norm = Math.max(...matrix.map((row) => row.reduce((sum, value) => sum + Math.abs(value), 0)));
  const squarings = Math.max(0, Math.ceil(Math.log2(Math.max(1, norm / 0.5))));
  const scaled = scaleArray(matrix, 1 / (2 ** squarings));
  let result = identity(dimension);
  let term = identity(dimension);
  for (let order = 1; order <= 80; order += 1) {
    term = scaleArray(multiplyArrays(term, scaled), 1 / order);
    result = addArrays(result, term);
    if (Math.max(...term.flat().map(Math.abs)) < 1e-16) break;
  }
  for (let index = 0; index < squarings; index += 1) result = multiplyArrays(result, result);
  return result;
}

function invertArray(matrix: readonly (readonly number[])[]): number[][] {
  return inverse(new Matrix(copyMatrix(matrix))).to2DArray();
}

function identity(size: number): number[][] {
  return Array.from({ length: size }, (_, row) =>
    Array.from({ length: size }, (_, column) => row === column ? 1 : 0));
}
function copyMatrix(matrix: readonly (readonly number[])[]): number[][] { return matrix.map((row) => Array.from(row)); }
function scaleArray(matrix: readonly (readonly number[])[], factor: number): number[][] {
  return matrix.map((row) => row.map((value) => value * factor));
}
function addArrays(left: readonly (readonly number[])[], right: readonly (readonly number[])[]): number[][] {
  return left.map((row, rowIndex) => row.map((value, column) => value + right[rowIndex][column]));
}
function multiplyArrays(left: readonly (readonly number[])[], right: readonly (readonly number[])[]): number[][] {
  const columns = right[0].length;
  return left.map((row) => Array.from({ length: columns }, (_, column) =>
    row.reduce((sum, value, index) => sum + value * right[index][column], 0)));
}
