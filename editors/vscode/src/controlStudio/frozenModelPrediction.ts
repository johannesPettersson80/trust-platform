import { delayedModelRealization } from "./delayedModelRealization";
import { solveLeastSquares } from "./numerics";
import type { GeneralModel } from "./generalModel";

/** Predict from frozen coefficients and measured inputs; only the initial state is estimated. */
export function frozenModelPrediction(model: GeneralModel, inputs: number[][], outputs: number[][], step: number): { start: number; values: number[][] } {
  const m = model.inputs.length, p = model.outputs.length;
  const u0 = model.operatingPoint?.inputs ?? Array(m).fill(0), y0 = model.operatingPoint?.outputs ?? Array(p).fill(0);
  const u = inputs.map(row => row.map((value, index) => value - u0[index]));
  const y = outputs.map(row => row.map((value, index) => value - y0[index]));
  const n = model.kind === "transfer-function" ? model.denominator.length - 1 : model.states.length;
  if (model.kind === "transfer-function" && n === 0 && model.delaySeconds === 0) {
    return { start: 0, values: inputs.map(row => [y0[0] + model.numerator[0] / model.denominator[0] * (row[0] - u0[0])]) };
  }
  if (model.kind === "discrete-state-space" && Math.abs(model.sampleTimeSeconds! - step) > 1e-8) throw new Error("Model and recording sample periods differ.");
  const realization = delayedModelRealization(model, step, Array(n).fill(0), u[0]);
  const advance = (state: readonly number[], input: readonly number[]) => realization.a.map((row, i) => dot(row, state) + dot(realization.b[i], input));
  const observe = (state: readonly number[], input: readonly number[]) => realization.c.map((row, i) => dot(row, state) + dot(realization.d[i], input) + y0[i]);
  let state = [...realization.initialState], start = 0;
  const order = (n + m) / (p + m);
  if (model.kind === "discrete-state-space" && model.delaySeconds === 0 && Number.isInteger(order)
    && model.states.every((entry, i) => entry.id === `history-${i}`)) {
    start = order - 1;
    state = [...Array.from({ length: order }, (_, lag) => y[start - lag]).flat(),
      ...Array.from({ length: order - 1 }, (_, lag) => u[start - lag - 1]).flat()];
  } else if (n) {
    const count = Math.max(3, n * 2);
    if (count >= inputs.length / 3) throw new Error("The recording is too short to separate initial-state estimation from model validation.");
    const basis = Array.from({ length: n }, (_, i) => realization.initialState.map((_, j) => Number(i === j)));
    let forced = [...state]; const matrix: number[][] = [], target: number[] = [];
    for (let k = 0; k < count; k++) {
      const zeroState = observe(forced, u[k]);
      for (let output = 0; output < p; output++) {
        matrix.push(basis.map(column => dot(realization.c[output], column)));
        target.push(outputs[k][output] - zeroState[output]);
      }
      forced = advance(forced, u[k]);
      basis.forEach((column, i) => { basis[i] = realization.a.map(row => dot(row, column)); });
    }
    const initial = solveLeastSquares(matrix, target, { absolute: 1e-10, relative: 1e-9, conditionLimit: 1e10 });
    if (initial.status !== "qualified") throw new Error("The model's initial state is not observable in the recording.");
    state.splice(0, n, ...initial.solution);
    for (let k = 0; k < count; k++) state = advance(state, u[k]);
    start = count;
  }
  const values: number[][] = [];
  for (let k = start; k < inputs.length; k++) {
    const value = observe(state, u[k]);
    if (!value.every(Number.isFinite)) throw new Error("Frozen model prediction exceeded finite values.");
    values.push(value); state = advance(state, u[k]);
  }
  return { start, values };
}

function dot(a: readonly number[], b: readonly number[]): number { return a.reduce((sum, value, i) => sum + value * b[i], 0); }
