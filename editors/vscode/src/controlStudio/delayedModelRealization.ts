import { transferFunctionStateSpace } from "./numerics/stateSpaceRealization";
import { discretizeStateSpace } from "./numerics";
import type { GeneralModel } from "./generalModel";

/** Exact held-input delay realization at the simulation sample instants. */
export function delayedModelRealization(model: GeneralModel, stepSeconds: number, initialState: readonly number[], initialInput: readonly number[]) {
  const matrices = model.kind === "transfer-function" ? transferFunctionStateSpace(model.numerator, model.denominator) : {
    a: model.a.map(row => [...row]), b: model.b.map(row => [...row]), c: model.c.map(row => [...row]), d: model.d.map(row => [...row]),
  };
  const n = matrices.a.length, m = model.inputs.length, outputs = model.outputs.length;
  if (initialState.length !== n || initialInput.length !== m) throw new Error("Delayed model initial coordinates must match its states and inputs.");
  const ratio = model.delaySeconds / stepSeconds;
  if (!Number.isFinite(ratio) || ratio < 0) throw new Error("Invalid model delay or sample period.");
  const nearest = Math.round(ratio);
  const whole = Math.abs(ratio - nearest) < 1e-10 ? nearest : Math.floor(ratio);
  const fractional = Math.abs(ratio - nearest) < 1e-10 ? 0 : ratio - whole;
  if (model.kind === "discrete-state-space" && fractional !== 0) throw new Error("A discrete model delay must be an integer number of its sample periods.");
  const history = whole + Number(fractional > 0);
  const states = n + history * m;
  if (states === 0 || states > 512) throw new Error("The exact delay history exceeds the supported 512-state realization budget.");
  const convert = (period: number) => {
    if (n === 0) return { a: [] as number[][], b: [] as number[][] };
    const result = discretizeStateSpace(matrices, period, "exact_zoh");
    if (result.status !== "qualified") throw new Error(`Exact held-input conversion failed: ${result.reason}.`);
    return result.model;
  };
  const full = model.kind === "discrete-state-space" ? matrices : convert(stepSeconds);
  const recent = fractional === 0 ? full : convert((1 - fractional) * stepSeconds);
  const a = Array.from({ length: states }, () => Array<number>(states).fill(0));
  const b = Array.from({ length: states }, () => Array<number>(m).fill(0));
  const c = Array.from({ length: outputs }, () => Array<number>(states).fill(0));
  const d = Array.from({ length: outputs }, () => Array<number>(m).fill(0));
  const inputCoefficient = (row: number, lag: number, input: number, value: number) => {
    if (lag === 0) b[row][input] += value;
    else a[row][n + (lag - 1) * m + input] += value;
  };
  for (let row = 0; row < n; row++) {
    for (let column = 0; column < n; column++) a[row][column] = full.a[row][column];
    for (let input = 0; input < m; input++) {
      inputCoefficient(row, whole, input, recent.b[row][input]);
      if (fractional > 0) inputCoefficient(row, whole + 1, input, full.b[row][input] - recent.b[row][input]);
    }
  }
  for (let lag = 0; lag < history; lag++) for (let input = 0; input < m; input++) {
    if (lag === 0) b[n + input][input] = 1;
    else a[n + lag * m + input][n + (lag - 1) * m + input] = 1;
  }
  for (let output = 0; output < outputs; output++) {
    for (let state = 0; state < n; state++) c[output][state] = matrices.c[output][state];
    for (let input = 0; input < m; input++) {
      if (history === 0) d[output][input] = matrices.d[output][input];
      else c[output][n + (history - 1) * m + input] = matrices.d[output][input];
    }
  }
  if (![...a.flat(), ...b.flat(), ...c.flat(), ...d.flat()].every(Number.isFinite)) throw new Error("Nonfinite exact-delay realization.");
  return { a, b, c, d, initialState: [...initialState, ...Array.from({ length: history }, () => [...initialInput]).flat()], history };
}
