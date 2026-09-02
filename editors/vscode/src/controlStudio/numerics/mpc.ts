import type { DenseMatrix } from "./contracts";
import { eigenvalues } from "./adapter";

interface LinearMpcProblem {
  a: DenseMatrix;
  b: DenseMatrix;
  state: readonly number[];
  reference: readonly (readonly number[])[];
  horizon: number;
  stateWeight: DenseMatrix;
  inputWeight: DenseMatrix;
  rateWeight: DenseMatrix;
  previousInput: readonly number[];
  inputMinimum: readonly number[];
  inputMaximum: readonly number[];
  rateMinimum: readonly number[];
  rateMaximum: readonly number[];
  stateMinimum: readonly number[];
  stateMaximum: readonly number[];
  fallbackInput: readonly number[];
  maximumIterations: number;
  feasibilityIterations: number;
  tolerance: number;
  deadlineMilliseconds: number;
}

interface HalfSpace { coefficients: number[]; maximum: number }

/** Dense projected-gradient QP for the small bounded horizons used by Control Studio lessons. */
export function solveLinearMpc(problem: LinearMpcProblem): any {
  const dimensions = validate(problem);
  if (!dimensions) return { status: "rejected", reason: "invalid-problem", firstMove: [...problem.fallbackInput], fallbackApplied: true };
  if (![problem.stateWeight, problem.inputWeight, problem.rateWeight]
    .every((matrix) => positiveSemidefinite(matrix, problem.tolerance))) {
    return { status: "rejected", reason: "non-convex-objective",
      firstMove: [...problem.fallbackInput], fallbackApplied: true };
  }
  const started = Date.now();
  const { states, inputs, decisions } = dimensions;
  const prediction = predictionModel(problem.a, problem.b, problem.state, problem.horizon, inputs);
  const constraints = buildConstraints(problem, prediction.free, prediction.sensitivity, states, inputs);
  let moves = Array.from({ length: problem.horizon }, () => Array.from(problem.previousInput)).flat();
  let projected = projectFeasible(moves, constraints, problem.feasibilityIterations, problem.tolerance);
  if (!projected.feasible) return fallback(problem, prediction, "infeasible", projected.maximumViolation);
  moves = projected.values;

  const gradientAtZero = objectiveGradient(problem, prediction, Array(decisions).fill(0));
  const hessian = Array.from({ length: decisions }, () => Array(decisions).fill(0) as number[]);
  for (let column = 0; column < decisions; column += 1) {
    const basis = Array(decisions).fill(0) as number[];
    basis[column] = 1;
    const gradient = objectiveGradient(problem, prediction, basis);
    for (let row = 0; row < decisions; row += 1) hessian[row][column] = gradient[row] - gradientAtZero[row];
  }
  const lipschitz = Math.max(problem.tolerance,
    ...hessian.map((row) => row.reduce((sum, value) => sum + Math.abs(value), 0)));
  let optimalityResidual = Number.POSITIVE_INFINITY;
  let iteration = 0;
  for (; iteration < problem.maximumIterations; iteration += 1) {
    if (Date.now() - started > problem.deadlineMilliseconds) {
      return fallback(problem, prediction, "deadline", maximumViolation(moves, constraints));
    }
    const gradient = addVectors(multiplyVector(hessian, moves), gradientAtZero);
    const trial = moves.map((value, index) => value - gradient[index] / lipschitz);
    projected = projectFeasible(trial, constraints, problem.feasibilityIterations, problem.tolerance);
    if (!projected.feasible) return fallback(problem, prediction, "infeasible", projected.maximumViolation);
    optimalityResidual = Math.hypot(...projected.values.map((value, index) => (value - moves[index]) * lipschitz));
    moves = projected.values;
    if (optimalityResidual <= problem.tolerance * Math.max(1, Math.hypot(...moves))) break;
  }
  if (iteration >= problem.maximumIterations) {
    return fallback(problem, prediction, "iteration-limit", maximumViolation(moves, constraints));
  }
  const predictedStates = predict(prediction, moves);
  const splitMoves = split(moves, inputs);
  return {
    status: "optimal",
    firstMove: splitMoves[0],
    moves: splitMoves,
    predictedStates,
    objective: objective(problem, predictedStates, splitMoves),
    maximumConstraintViolation: maximumViolation(moves, constraints),
    optimalityResidual,
    iterations: iteration + 1,
    elapsedMilliseconds: Date.now() - started,
    fallbackApplied: false,
  };
}

function predictionModel(a: DenseMatrix, b: DenseMatrix, initial: readonly number[], horizon: number, inputs: number) {
  const states = a.length;
  const decisions = horizon * inputs;
  const free: number[][] = [];
  const sensitivity: number[][][] = [];
  let currentFree = Array.from(initial);
  let currentSensitivity = Array.from({ length: states }, () => Array(decisions).fill(0) as number[]);
  for (let step = 0; step < horizon; step += 1) {
    currentFree = multiplyVector(a, currentFree);
    currentSensitivity = multiply(a, currentSensitivity);
    for (let state = 0; state < states; state += 1) {
      for (let input = 0; input < inputs; input += 1) {
        currentSensitivity[state][step * inputs + input] += b[state][input];
      }
    }
    free.push([...currentFree]);
    sensitivity.push(currentSensitivity.map((row) => [...row]));
  }
  return { free, sensitivity };
}

function buildConstraints(problem: LinearMpcProblem, free: readonly number[][],
  sensitivity: readonly number[][][], states: number, inputs: number): HalfSpace[] {
  const decisions = problem.horizon * inputs;
  const result: HalfSpace[] = [];
  for (let step = 0; step < problem.horizon; step += 1) {
    for (let input = 0; input < inputs; input += 1) {
      const index = step * inputs + input;
      const unit = Array(decisions).fill(0) as number[];
      unit[index] = 1;
      result.push({ coefficients: unit, maximum: problem.inputMaximum[input] });
      result.push({ coefficients: unit.map((value) => -value), maximum: -problem.inputMinimum[input] });
      const difference = [...unit];
      if (step > 0) difference[(step - 1) * inputs + input] = -1;
      result.push({ coefficients: difference, maximum: problem.rateMaximum[input]
        + (step === 0 ? problem.previousInput[input] : 0) });
      result.push({ coefficients: difference.map((value) => -value), maximum: -problem.rateMinimum[input]
        - (step === 0 ? problem.previousInput[input] : 0) });
    }
    for (let state = 0; state < states; state += 1) {
      result.push({ coefficients: [...sensitivity[step][state]], maximum: problem.stateMaximum[state] - free[step][state] });
      result.push({ coefficients: sensitivity[step][state].map((value) => -value),
        maximum: free[step][state] - problem.stateMinimum[state] });
    }
  }
  return result;
}

function objectiveGradient(problem: LinearMpcProblem, prediction: any, decisions: readonly number[]): number[] {
  const inputs = problem.b[0].length;
  const states = predict(prediction, decisions);
  const moves = split(decisions, inputs);
  const gradient = Array(decisions.length).fill(0) as number[];
  for (let step = 0; step < problem.horizon; step += 1) {
    const error = subtractVectors(states[step], problem.reference[step]);
    const weightedState = multiplyVector(problem.stateWeight, error);
    for (let decision = 0; decision < decisions.length; decision += 1) {
      gradient[decision] += 2 * dot(prediction.sensitivity[step].map((row: number[]) => row[decision]), weightedState);
    }
    const weightedInput = multiplyVector(problem.inputWeight, moves[step]);
    for (let input = 0; input < inputs; input += 1) gradient[step * inputs + input] += 2 * weightedInput[input];
    const previous = step === 0 ? problem.previousInput : moves[step - 1];
    const rate = subtractVectors(moves[step], previous);
    const weightedRate = multiplyVector(problem.rateWeight, rate);
    for (let input = 0; input < inputs; input += 1) {
      gradient[step * inputs + input] += 2 * weightedRate[input];
      if (step > 0) gradient[(step - 1) * inputs + input] -= 2 * weightedRate[input];
    }
  }
  return gradient;
}

function objective(problem: LinearMpcProblem, states: readonly number[][], moves: readonly number[][]): number {
  let value = 0;
  for (let step = 0; step < problem.horizon; step += 1) {
    const error = subtractVectors(states[step], problem.reference[step]);
    value += dot(error, multiplyVector(problem.stateWeight, error));
    value += dot(moves[step], multiplyVector(problem.inputWeight, moves[step]));
    const previous = step === 0 ? problem.previousInput : moves[step - 1];
    const rate = subtractVectors(moves[step], previous);
    value += dot(rate, multiplyVector(problem.rateWeight, rate));
  }
  return value;
}

function projectFeasible(initial: readonly number[], constraints: readonly HalfSpace[], iterations: number, tolerance: number) {
  const values = [...initial];
  for (let sweep = 0; sweep < iterations; sweep += 1) {
    for (const constraint of constraints) {
      const violation = dot(constraint.coefficients, values) - constraint.maximum;
      if (violation > 0) {
        const norm = dot(constraint.coefficients, constraint.coefficients);
        if (norm <= Number.EPSILON) continue;
        for (let index = 0; index < values.length; index += 1) {
          values[index] -= violation * constraint.coefficients[index] / norm;
        }
      }
    }
    const maximum = maximumViolation(values, constraints);
    if (maximum <= tolerance) return { feasible: true, values, maximumViolation: maximum };
  }
  return { feasible: false, values, maximumViolation: maximumViolation(values, constraints) };
}

function maximumViolation(values: readonly number[], constraints: readonly HalfSpace[]): number {
  return Math.max(0, ...constraints.map((constraint) => dot(constraint.coefficients, values) - constraint.maximum));
}

function predict(prediction: any, decisions: readonly number[]): number[][] {
  return prediction.free.map((free: number[], step: number) =>
    addVectors(free, multiplyVector(prediction.sensitivity[step], decisions)));
}

function fallback(problem: LinearMpcProblem, prediction: any, status: string, violation: number): any {
  const moves = Array.from({ length: problem.horizon }, () => [...problem.fallbackInput]);
  const flat = moves.flat();
  return { status, reason: status, firstMove: [...problem.fallbackInput], moves,
    predictedStates: predict(prediction, flat), maximumConstraintViolation: violation,
    optimalityResidual: Number.POSITIVE_INFINITY, fallbackApplied: true };
}

function validate(problem: LinearMpcProblem): { states: number; inputs: number; decisions: number } | undefined {
  const states = problem.a.length;
  const inputs = problem.b[0]?.length ?? 0;
  const square = (matrix: DenseMatrix, size: number) => matrix.length === size && matrix.every((row) => row.length === size);
  const vector = (values: readonly number[], size: number) => values.length === size && values.every(Number.isFinite);
  if (states === 0 || inputs === 0 || !square(problem.a, states)
      || problem.b.length !== states || problem.b.some((row) => row.length !== inputs)
      || !square(problem.stateWeight, states) || !square(problem.inputWeight, inputs) || !square(problem.rateWeight, inputs)
      || !vector(problem.state, states) || !vector(problem.previousInput, inputs)
      || !vector(problem.inputMinimum, inputs) || !vector(problem.inputMaximum, inputs)
      || !vector(problem.rateMinimum, inputs) || !vector(problem.rateMaximum, inputs)
      || !vector(problem.stateMinimum, states) || !vector(problem.stateMaximum, states)
      || !vector(problem.fallbackInput, inputs) || !Number.isInteger(problem.horizon) || problem.horizon < 1
      || problem.reference.length !== problem.horizon || problem.reference.some((row) => !vector(row, states))
      || !Number.isInteger(problem.maximumIterations) || problem.maximumIterations < 1
      || !Number.isInteger(problem.feasibilityIterations) || problem.feasibilityIterations < 1
      || !Number.isFinite(problem.tolerance) || problem.tolerance <= 0
      || !Number.isFinite(problem.deadlineMilliseconds) || problem.deadlineMilliseconds <= 0
      || ![...problem.a.flat(), ...problem.b.flat(), ...problem.stateWeight.flat(),
        ...problem.inputWeight.flat(), ...problem.rateWeight.flat()].every(Number.isFinite)
      || problem.inputMinimum.some((value, index) => value > problem.inputMaximum[index])
      || problem.rateMinimum.some((value, index) => value > problem.rateMaximum[index])
      || problem.stateMinimum.some((value, index) => value > problem.stateMaximum[index])) return undefined;
  return { states, inputs, decisions: problem.horizon * inputs };
}

function split(values: readonly number[], width: number): number[][] {
  return Array.from({ length: values.length / width }, (_, index) => values.slice(index * width, (index + 1) * width));
}
function multiply(left: DenseMatrix, right: DenseMatrix): number[][] {
  return left.map((row) => Array.from({ length: right[0].length }, (_, column) =>
    row.reduce((sum, value, index) => sum + value * right[index][column], 0)));
}
function multiplyVector(matrix: DenseMatrix, vector: readonly number[]): number[] {
  return matrix.map((row) => dot(row, vector));
}
function addVectors(left: readonly number[], right: readonly number[]): number[] {
  return left.map((value, index) => value + right[index]);
}
function subtractVectors(left: readonly number[], right: readonly number[]): number[] {
  return left.map((value, index) => value - right[index]);
}
function dot(left: readonly number[], right: readonly number[]): number {
  return left.reduce((sum, value, index) => sum + value * right[index], 0);
}

function positiveSemidefinite(matrix: DenseMatrix, tolerance: number): boolean {
  if (!matrix.every((row, rowIndex) => row.every((value, column) =>
    Math.abs(value - matrix[column][rowIndex]) <= tolerance))) return false;
  const spectrum = eigenvalues(matrix, {
    absolute: tolerance,
    relative: Math.max(tolerance, 1e-12),
    conditionLimit: 1 / Math.max(tolerance, 1e-12),
  });
  return spectrum.status === "qualified"
    && spectrum.values.every((value) => Math.abs(value.imaginary) <= tolerance && value.real >= -tolerance);
}
