import { decodeControllerInitialization, type ControllerInitialization } from "./controllerInitialization";
import { connectedModelChannels } from "./loopChannels";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { ParameterBinding } from "./parameterBinding";
import type { GeneralModel, GeneralStateSpaceModel } from "./generalModel";
import { discretizeStateSpace, eigenvalues, placeStateFeedbackPoles, solveDiscreteLqr, designDiscreteLqg, solveLeastSquares } from "./numerics";
import type { DenseMatrix } from "./numerics";
import type { StateDesignPreferences } from "./stateDesignPreferences";

export type StateControllerFamily = "state-feedback" | "LQR" | "LQG";
export interface StateControllerBinding {
  family: StateControllerFamily;
  convention: "sampled-deviation-negative-feedback";
  gainSymbols: readonly (readonly string[])[];
  referenceGainSymbols: readonly (readonly string[])[];
  observerGainSymbols?: readonly (readonly string[])[];
  desiredPoles?: readonly number[];
  stateWeights?: DenseMatrix;
  inputWeights?: DenseMatrix;
  processNoise?: DenseMatrix;
  measurementNoise?: DenseMatrix;
  estimatorModel?: { a: DenseMatrix; b: DenseMatrix; c: DenseMatrix; sampleTimeSeconds: number };
  initialization?: ControllerInitialization;
  estimatorStateSymbols?: readonly string[];
}
export interface StateControllerDesign {
  status: "provisional" | "exploratory";
  candidate: { applied: false; structure: StateControllerFamily; parameters: Readonly<Record<string, number>>; proportionalGain?: never };
  selectedModel: { revision: string; provenance: string };
  actualSTVerify: true;
  scope: string;
  closedLoopPoles: readonly { real: number; imaginary: number }[];
  horizon?: { steps: number; policy: "repeat-first-finite-gain" | "pinned-fixed-gain"; gainSequence: readonly DenseMatrix[]; riccatiSequence: readonly DenseMatrix[] };
}
const tolerance = { absolute: 1e-10, relative: 1e-9, conditionLimit: 1e10 };
export function decodeStateControllerBinding(value: unknown, binding?: ParameterBinding): StateControllerBinding | undefined {
  if (!record(value) || value.convention !== "sampled-deviation-negative-feedback" || !["state-feedback", "LQR", "LQG"].includes(String(value.family)) || !binding) return undefined;
  const symbols = (value: unknown): value is string[][] => Array.isArray(value) && value.length > 0 && value.length <= 16
    && value.every(row => Array.isArray(row) && row.length > 0 && row.length <= 32 && row.every(symbol => typeof symbol === "string" && binding.parameters.some(parameter => parameter.symbol === symbol)));
  if (!symbols(value.gainSymbols) || !symbols(value.referenceGainSymbols)
    || value.family === "LQG" && !symbols(value.observerGainSymbols)
    || value.observerGainSymbols !== undefined && (value.family !== "LQG" || !symbols(value.observerGainSymbols))) return undefined;
  if (value.initialization !== undefined && !decodeControllerInitialization(value.initialization)) return undefined;
  if (value.family === "LQG" && value.initialization !== undefined) {
    if (!Array.isArray(value.estimatorStateSymbols) || value.estimatorStateSymbols.length !== (value.observerGainSymbols as string[][]).length
      || !value.estimatorStateSymbols.every(symbol => typeof symbol === "string" && decodeControllerInitialization(value.initialization)!.states.some(state => state.symbol === symbol && state.iec_type !== "BOOL"))) return undefined;
  }
  const all = [...value.gainSymbols.flat(), ...value.referenceGainSymbols.flat(), ...((value.observerGainSymbols as string[][] | undefined)?.flat() ?? [])];
  if (new Set(all).size !== all.length) return undefined;
  if (value.family === "LQG") {
    const estimator = value.estimatorModel;
    const matrix = (value: unknown) => Array.isArray(value) && value.length > 0 && value.length <= 32
      && value.every(row => Array.isArray(row) && row.length > 0 && row.length <= 32 && row.every(finite));
    if (!record(estimator) || !finite(estimator.sampleTimeSeconds) || estimator.sampleTimeSeconds <= 0
      || !matrix(estimator.a) || !matrix(estimator.b) || !matrix(estimator.c)) return undefined;
  }
  for (const key of ["stateWeights", "inputWeights", "processNoise", "measurementNoise"]) {
    const matrix = value[key];
    if (matrix !== undefined && (!Array.isArray(matrix) || !matrix.length || matrix.length > 32
      || !matrix.every(row => Array.isArray(row) && row.length === matrix.length && row.every(finite)))) return undefined;
  }
  if (value.desiredPoles !== undefined && (!Array.isArray(value.desiredPoles) || !value.desiredPoles.length || !value.desiredPoles.every(finite))) return undefined;
  return structuredClone(value) as unknown as StateControllerBinding;
}

/** Uses the existing numerical owners; no new pole-placement or Riccati solver. */
export function designStateController(loop: ControlLoopDefinition, model: GeneralModel, responseTimeSeconds: number, selectedFamily?: StateControllerFamily, override?: StateDesignPreferences): StateControllerDesign {
  const declared = decodeStateControllerBinding(loop.controller.stateFeedback, loop.parameterBinding);
  const definition = declared ? { ...declared, ...(override ?? {}), family: selectedFamily ?? declared.family } : undefined;
  if (declared && ((declared.family === "LQG") !== (definition!.family === "LQG"))) throw new Error("Changing estimator structure requires its own typed ST adapter.");
  if (!definition || model.kind === "transfer-function" || model.delaySeconds !== 0) throw new Error("State-controller design requires an explicit coefficient binding and a delay-free state-space model.");
  const n = model.states.length, m = model.inputs.length, p = model.outputs.length;
  const sampled = sampleModel(model, loop.controller.taskPeriodMs / 1000);
  let gain: DenseMatrix, observer: DenseMatrix | undefined;
  let horizon: StateControllerDesign["horizon"];
  if (definition.family === "state-feedback") {
    const desired = definition.desiredPoles ?? Array.from({ length: n }, (_, index) => Math.exp(-(4 + index / Math.max(1, n)) * sampled.sampleTimeSeconds / responseTimeSeconds));
    if (desired.length !== n || desired.some(pole => !Number.isFinite(pole) || Math.abs(pole) >= 1)) throw new Error("Choose one strict unit-circle pole for every model state.");
    const result = placeStateFeedbackPoles(sampled.a, sampled.b, desired, tolerance);
    if (result.status !== "qualified") throw new Error(`State-feedback design: ${result.reason}.`);
    gain = result.gain;
  } else {
    const q = definition.stateWeights ?? diagonal(model.states.map(state => 1 / (state.operatingRange.maximum - state.operatingRange.minimum) ** 2));
    const r = definition.inputWeights ?? diagonal(model.inputs.map(input => 1 / (input.operatingRange.maximum - input.operatingRange.minimum) ** 2));
    positiveWeight(q, n, false); positiveWeight(r, m, true);
    if (definition.family === "LQG") {
      const declaredModel = definition.estimatorModel!;
      const sameMatrix = (left: DenseMatrix, right: DenseMatrix) => left.length === right.length
        && left.every((row, i) => row.length === right[i].length && row.every((value, j) => value === right[i][j]));
      if (declaredModel.sampleTimeSeconds !== sampled.sampleTimeSeconds || !sameMatrix(declaredModel.a, sampled.a)
        || !sameMatrix(declaredModel.b, sampled.b) || !sameMatrix(declaredModel.c, sampled.c)) {
        throw new Error("The selected model differs from the declared ST estimator model. Update its coefficient binding or select the matching model before LQG design.");
      }
      if (model.d.some(row => row.some(value => value !== 0))) throw new Error("The LQG adapter needs a declared measurement feedthrough correction for a model with nonzero D.");
      if (!definition.processNoise || !definition.measurementNoise) throw new Error("Supply justified process and measurement covariance before LQG design.");
      positiveWeight(definition.processNoise, n, false); positiveWeight(definition.measurementNoise, p, true);
      const result = designDiscreteLqg({ ...sampled, q, r, processNoise: definition.processNoise, measurementNoise: definition.measurementNoise, finiteHorizon: definition.finiteHorizon }, tolerance);
      if (result.status !== "qualified") throw new Error(`LQG design: ${result.reason}.`);
      gain = result.lqr.gain; observer = result.kalman.gain;
      if (definition.finiteHorizon) horizon = { steps: definition.finiteHorizon.steps, policy: "repeat-first-finite-gain", gainSequence: result.lqr.gainSequence, riccatiSequence: result.lqr.riccatiSequence };
    } else {
      const result = solveDiscreteLqr(sampled.a, sampled.b, q, r, tolerance, definition.finiteHorizon);
      if (result.status !== "qualified") throw new Error(`LQR design: ${result.reason}.`);
      gain = result.gain;
      if (definition.finiteHorizon) horizon = { steps: definition.finiteHorizon.steps, policy: "repeat-first-finite-gain", gainSequence: result.gainSequence, riccatiSequence: result.riccatiSequence };
    }
  }
  const connected = connectedModelChannels(loop, model);
  const controlled = connected.outputs.flatMap((output, index) => output.reference ? [index] : []);
  if (m !== controlled.length) throw new Error("Independent reference tracking requires one controlled objective per available input; auxiliary measurements are retained separately.");
  const equilibrium = [...sampled.a.map((row, i) => [...row.map((value, j) => value - Number(i === j)), ...sampled.b[i]]),
    ...controlled.map(i => [...model.c[i], ...model.d[i]])];
  const referenceGain = Array.from({ length: m }, () => Array<number>(controlled.length).fill(0));
  for (let output = 0; output < controlled.length; output++) {
    const equilibriumState = solveLeastSquares(equilibrium, [...Array<number>(n).fill(0), ...Array.from({ length: controlled.length }, (_, i) => Number(i === output))], tolerance);
    if (equilibriumState.status !== "qualified") throw new Error("The selected output objectives have insufficient steady control authority.");
    for (let input = 0; input < m; input++) referenceGain[input][output] = equilibriumState.solution[n + input]
      + gain[input].reduce((sum, value, state) => sum + value * equilibriumState.solution[state], 0);
  }
  const values: Record<string, number> = {};
  const map = (symbols: readonly (readonly string[])[], coefficients: DenseMatrix, unit: (row: number, column: number) => string) => {
    if (symbols.length !== coefficients.length || symbols.some((row, i) => row.length !== coefficients[i].length)) throw new Error("ST coefficient layout does not match model coordinates.");
    coefficients.forEach((row, i) => row.forEach((value, j) => {
      const parameter = loop.parameterBinding!.parameters.find(parameter => parameter.symbol === symbols[i][j])!;
      if (parameter.unit !== unit(i, j)) throw new Error(`Coefficient ${parameter.symbol} unit does not match its model coordinates.`);
      const realized = parameter.iec_type === "REAL" ? Math.fround(value) : value;
      if (!Number.isFinite(realized) || realized < parameter.minimum || realized > parameter.maximum) throw new Error(`Calculated coefficient ${parameter.symbol} is outside its representable bounds.`);
      values[parameter.symbol] = realized;
    }));
  };
  map(definition.gainSymbols, gain, (i, j) => ratioUnit(model.inputs[i].unit, model.states[j].unit));
  map(definition.referenceGainSymbols, referenceGain, (i, j) => ratioUnit(model.inputs[i].unit, model.outputs[controlled[j]].unit));
  if (observer) map(definition.observerGainSymbols!, observer, (i, j) => ratioUnit(model.states[i].unit, model.outputs[j].unit));
  const closedLoopPoles = stateControllerPoles(loop, model, values);
  return { status: "provisional", candidate: { applied: false, structure: definition.family, parameters: values },
    selectedModel: { revision: model.revision, provenance: `declared-${model.source}-${model.validation}` }, actualSTVerify: true, closedLoopPoles, ...(horizon ? { horizon } : {}),
    scope: "Mapped sampled state-feedback coefficients in declared coordinates; actual-ST verification and a separately declared initialization contract govern application." };
}
/** Recalculate active poles after REAL rounding or manual coefficient pins. */
export function stateControllerPoles(loop: ControlLoopDefinition, model: GeneralModel, values: Readonly<Record<string, number>>) {
  const definition = decodeStateControllerBinding(loop.controller.stateFeedback, loop.parameterBinding);
  if (!definition || model.kind === "transfer-function") throw new Error("State-controller coordinates are unavailable.");
  const sampled = sampleModel(model, loop.controller.taskPeriodMs / 1000);
  const gain = definition.gainSymbols.map(row => row.map(symbol => values[symbol]));
  if (gain.length !== model.inputs.length || gain.some(row => row.length !== model.states.length || row.some(value => !Number.isFinite(value)))) throw new Error("State-feedback gain coordinates do not match the model.");
  const closed = sampled.a.map((row, i) => row.map((value, j) => value - sampled.b[i].reduce((sum, b, k) => sum + b * gain[k][j], 0)));
  const result = eigenvalues(closed, tolerance);
  if (result.status !== "qualified") throw new Error("State-controller pole calculation failed.");
  const poles = [...result.values];
  if (definition.family === "LQG") {
    const observer = definition.observerGainSymbols!.map(row => row.map(symbol => values[symbol]));
    if (observer.length !== model.states.length || observer.some(row => row.length !== model.outputs.length || row.some(value => !Number.isFinite(value)))) throw new Error("Observer gain coordinates do not match the model.");
    const correction = sampled.a.map((_, i) => sampled.a.map((_, j) => Number(i === j)
      - observer[i].reduce((sum, value, output) => sum + value * sampled.c[output][j], 0)));
    const estimator = correction.map(row => sampled.a.map((_, j) => row.reduce((sum, value, k) => sum + value * sampled.a[k][j], 0)));
    const estimated = eigenvalues(estimator, tolerance);
    if (estimated.status !== "qualified") throw new Error("Observer pole calculation failed.");
    poles.push(...estimated.values);
  }
  if (poles.some(pole => Math.hypot(pole.real, pole.imaginary) >= 1)) throw new Error("Mapped controller/observer coefficients do not establish strict sampled stability.");
  return poles;
}

/** The unit of a coefficient that maps a quantity in `denominator` units to one in `numerator` units. */
export function ratioUnit(numerator: string, denominator: string): string {
  return numerator === denominator ? "1" : `${numerator}/${denominator}`;
}

export function sampleModel(model: GeneralStateSpaceModel, period: number) {
  if (model.kind === "discrete-state-space") {
    if (Math.abs(model.sampleTimeSeconds! - period) > 1e-9) throw new Error("State controller and model sample periods differ.");
    return { a: model.a, b: model.b, c: model.c, d: model.d, sampleTimeSeconds: period };
  }
  const result = discretizeStateSpace(model, period, "exact_zoh");
  if (result.status !== "qualified") throw new Error(`Exact model discretization: ${result.reason}.`);
  return result.model;
}
function diagonal(values: readonly number[]): number[][] { return values.map((value, i) => values.map((_, j) => i === j ? value : 0)); }
function positiveWeight(matrix: DenseMatrix, size: number, strict: boolean): void {
  if (matrix.length !== size || matrix.some((row, i) => row.length !== size || row.some((value, j) => !Number.isFinite(value) || Math.abs(value - matrix[j]?.[i]) > 1e-10))) throw new Error("Design weights/covariances must be finite symmetric matrices in declared coordinates.");
  const result = eigenvalues(matrix, tolerance);
  if (result.status !== "qualified" || result.values.some(value => Math.abs(value.imaginary) > 1e-9 || (strict ? value.real <= 0 : value.real < -1e-10))) throw new Error("Design weights/covariances must satisfy the required positive-(semi)definite condition.");
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function finite(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value); }
