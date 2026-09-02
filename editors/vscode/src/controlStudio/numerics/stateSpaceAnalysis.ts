import { analyzeMatrix, eigenDecomposition, solveLeastSquares } from "./adapter";
import type { ComplexValue, ContinuousStateSpaceModel, DenseMatrix, NumericalTolerance } from "./contracts";

interface Complex { real: number; imaginary: number }

export function analyzeStateSpace(model: ContinuousStateSpaceModel, tolerance: NumericalTolerance): any {
  const dimensions = validateModel(model);
  if (!dimensions) return { status: "rejected", reason: "inconsistent-dimensions" };
  const eigen = eigenDecomposition(model.a, tolerance);
  if (eigen.status !== "qualified") return eigen;
  const controllabilityMatrix = controllability(model.a, model.b);
  const observabilityMatrix = observability(model.a, model.c);
  const controllabilityAnalysis = analyzeMatrix(controllabilityMatrix, tolerance);
  const observabilityAnalysis = analyzeMatrix(observabilityMatrix, tolerance);
  const controllable = controllabilityAnalysis.rank === dimensions.states;
  const observable = observabilityAnalysis.rank === dimensions.states;
  return {
    status: "qualified",
    model: copyModel(model),
    eigenvalues: eigen.values,
    modes: eigen.modes,
    controllability: { matrix: controllabilityMatrix, ...controllabilityAnalysis },
    observability: { matrix: observabilityMatrix, ...observabilityAnalysis },
    controllable,
    observable,
    stabilisable: controllable || eigen.values.every((value: ComplexValue) =>
      value.real < -tolerance.absolute || pbhControllable(model.a, model.b, value, tolerance)),
    detectable: observable || eigen.values.every((value: ComplexValue) =>
      value.real < -tolerance.absolute || pbhObservable(model.a, model.c, value, tolerance)),
  };
}

export function modalCoordinates(modes: readonly any[], state: readonly number[], tolerance: NumericalTolerance): any {
  if (modes.length !== state.length || !state.every(Number.isFinite)
      || modes.some((mode) => Math.abs(mode.eigenvalue.imaginary) > tolerance.absolute)) {
    return { status: "rejected", reason: "complex-or-inconsistent-modes" };
  }
  const vectors = state.map((_, row) => modes.map((mode) => mode.vector[row].real));
  const solved = solveLeastSquares(vectors, state, tolerance);
  return solved.status === "qualified"
    ? { status: "qualified", values: solved.solution }
    : solved;
}

export function modalReconstruction(modes: readonly any[], coordinates: readonly number[]): any {
  if (modes.length !== coordinates.length || !coordinates.every(Number.isFinite)
      || modes.length === 0 || modes.some((mode) => Math.abs(mode.eigenvalue.imaginary) > 1e-12)) {
    return { status: "rejected", reason: "complex-or-inconsistent-modes" };
  }
  const state = modes[0].vector.map((_: unknown, row: number) =>
    modes.reduce((sum: number, mode: any, index: number) => sum + mode.vector[row].real * coordinates[index], 0));
  return { status: "qualified", state };
}

export function stateDerivative(
  model: ContinuousStateSpaceModel,
  state: readonly number[],
  input: readonly number[]
): number[] {
  return model.a.map((row, index) => dot(row, state) + dot(model.b[index], input));
}

function controllability(a: DenseMatrix, b: DenseMatrix): number[][] {
  const blocks: number[][][] = [copyMatrix(b)];
  for (let power = 1; power < a.length; power += 1) blocks.push(multiply(a, blocks[power - 1]));
  return Array.from({ length: a.length }, (_, row) => blocks.flatMap((block) => block[row]));
}

function observability(a: DenseMatrix, c: DenseMatrix): number[][] {
  const blocks: number[][][] = [copyMatrix(c)];
  for (let power = 1; power < a.length; power += 1) blocks.push(multiply(blocks[power - 1], a));
  return blocks.flatMap((block) => block);
}

function pbhControllable(a: DenseMatrix, b: DenseMatrix, eigenvalue: ComplexValue, tolerance: NumericalTolerance): boolean {
  const complex = a.map((row, rowIndex) => [
    ...row.map((value, column) => ({ real: (rowIndex === column ? eigenvalue.real : 0) - value,
      imaginary: rowIndex === column ? eigenvalue.imaginary : 0 })),
    ...b[rowIndex].map((value) => ({ real: value, imaginary: 0 })),
  ]);
  return complexRank(complex, tolerance) === a.length;
}

function pbhObservable(a: DenseMatrix, c: DenseMatrix, eigenvalue: ComplexValue, tolerance: NumericalTolerance): boolean {
  const complex = [
    ...a.map((row, rowIndex) => row.map((value, column) => ({
      real: (rowIndex === column ? eigenvalue.real : 0) - value,
      imaginary: rowIndex === column ? eigenvalue.imaginary : 0,
    }))),
    ...c.map((row) => row.map((value) => ({ real: value, imaginary: 0 }))),
  ];
  return complexRank(complex, tolerance) === a.length;
}

function complexRank(matrix: readonly (readonly Complex[])[], tolerance: NumericalTolerance): number {
  const work = matrix.map((row) => row.map((value) => ({ ...value })));
  let rank = 0;
  for (let column = 0; column < work[0].length && rank < work.length; column += 1) {
    let pivot = rank;
    for (let row = rank + 1; row < work.length; row += 1) {
      if (absolute(work[row][column]) > absolute(work[pivot][column])) pivot = row;
    }
    if (absolute(work[pivot][column]) <= tolerance.absolute) continue;
    [work[rank], work[pivot]] = [work[pivot], work[rank]];
    for (let row = rank + 1; row < work.length; row += 1) {
      const factor = divide(work[row][column], work[rank][column]);
      for (let current = column; current < work[row].length; current += 1) {
        work[row][current] = subtract(work[row][current], multiplyComplex(factor, work[rank][current]));
      }
    }
    rank += 1;
  }
  return rank;
}

function validateModel(model: ContinuousStateSpaceModel): { states: number; inputs: number } | undefined {
  const states = model.a.length;
  const inputs = model.b[0]?.length ?? 0;
  const outputs = model.c.length;
  if (states === 0 || inputs === 0 || outputs === 0 || model.a.some((row) => row.length !== states)
      || model.b.length !== states || model.b.some((row) => row.length !== inputs)
      || model.c.some((row) => row.length !== states)
      || model.d.length !== outputs || model.d.some((row) => row.length !== inputs)
      || ![...model.a.flat(), ...model.b.flat(), ...model.c.flat(), ...model.d.flat()].every(Number.isFinite)) {
    return undefined;
  }
  return { states, inputs };
}

function copyModel(model: ContinuousStateSpaceModel): ContinuousStateSpaceModel {
  return { a: copyMatrix(model.a), b: copyMatrix(model.b), c: copyMatrix(model.c), d: copyMatrix(model.d) };
}
function copyMatrix(matrix: DenseMatrix): number[][] { return matrix.map((row) => Array.from(row)); }
function multiply(left: DenseMatrix, right: DenseMatrix): number[][] {
  return left.map((row) => Array.from({ length: right[0].length }, (_, column) =>
    row.reduce((sum, value, index) => sum + value * right[index][column], 0)));
}
function dot(left: readonly number[], right: readonly number[]): number {
  return left.reduce((sum, value, index) => sum + value * right[index], 0);
}
function absolute(value: Complex): number { return Math.hypot(value.real, value.imaginary); }
function subtract(left: Complex, right: Complex): Complex {
  return { real: left.real - right.real, imaginary: left.imaginary - right.imaginary };
}
function multiplyComplex(left: Complex, right: Complex): Complex {
  return { real: left.real * right.real - left.imaginary * right.imaginary,
    imaginary: left.real * right.imaginary + left.imaginary * right.real };
}
function divide(left: Complex, right: Complex): Complex {
  const denominator = right.real ** 2 + right.imaginary ** 2;
  return { real: (left.real * right.real + left.imaginary * right.imaginary) / denominator,
    imaginary: (left.imaginary * right.real - left.real * right.imaginary) / denominator };
}
