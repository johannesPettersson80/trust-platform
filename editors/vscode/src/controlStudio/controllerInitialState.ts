import type { ControlLoopDefinition } from "./loopDefinition";
import type { GeneralModel } from "./generalModel";
import { roundedTransferValue, type TransferMember } from "./proportionalTransfer";
import { connectedModelChannels } from "./loopChannels";

export type ControllerInitialValue = number
  | { source: "model-input" | "model-output" | "model-state" | "model-input-is"; index: number; value?: number }
  | { source: "pid-integral"; index: number; inputIndex: number; gainSymbol: string; weightSymbol?: string; weight?: number }
  | { source: "weighted-error"; index: number; weightSymbol?: string; weight?: number }
  /** A model input times a controller parameter, such as a feedforward state `K_ff·d` (Specification 35 §17.12). */
  | { source: "scaled-model-input"; index: number; gainSymbol: string }
  /** Row `index` of D⁻¹·u: a decoupled group member's output at the model's operating inputs (Specification 35 §17.15). */
  | { source: "decoupled-model-input"; index: number; matrixSymbols: readonly (readonly string[])[] }
  | { source: "notch-history"; index: number; denominatorSymbols: readonly [string, string] };

export interface ControllerInitialization {
  kind: "declared-state-v1";
  states: readonly (TransferMember & {
    initial: ControllerInitialValue;
    postScan: "unchanged" | "bounded";
  })[];
}
export function decodeControllerInitialization(value: unknown): ControllerInitialization | undefined {
  if (!record(value) || value.kind !== "declared-state-v1" || !Array.isArray(value.states) || value.states.length > 32) return undefined;
  const names = new Set<string>();
  for (const state of value.states) {
    if (!record(state) || typeof state.symbol !== "string" || !state.symbol || names.has(state.symbol)
      || !["REAL", "LREAL", "BOOL"].includes(String(state.iec_type)) || typeof state.unit !== "string" || !state.unit
      || !finite(state.minimum) || !finite(state.maximum) || state.minimum > state.maximum
      || !["unchanged", "bounded"].includes(String(state.postScan))) return undefined;
    if (state.iec_type === "BOOL" && (state.unit !== "1" || state.minimum !== 0 || state.maximum !== 1)) return undefined;
    const initial = state.initial;
    if (!finite(initial)) {
      if (!record(initial) || !["model-input", "model-output", "model-state", "model-input-is", "pid-integral", "weighted-error", "notch-history", "scaled-model-input", "decoupled-model-input"].includes(String(initial.source))
        || !Number.isSafeInteger(initial.index) || (initial.index as number) < 0) return undefined;
      if (initial.source === "model-input-is" && !finite(initial.value)) return undefined;
      if (initial.source === "scaled-model-input" && (typeof initial.gainSymbol !== "string" || !initial.gainSymbol)) return undefined;
      if (initial.source === "decoupled-model-input" && !(Array.isArray(initial.matrixSymbols) && initial.matrixSymbols.length > 0
        && initial.matrixSymbols.length <= 8 && initial.matrixSymbols.every(row => Array.isArray(row) && row.length === (initial.matrixSymbols as unknown[]).length
          && row.every(symbol => typeof symbol === "string" && !!symbol)) && (initial.index as number) < initial.matrixSymbols.length)) return undefined;
      if (initial.source === "pid-integral" && (!Number.isSafeInteger(initial.inputIndex) || (initial.inputIndex as number) < 0 || typeof initial.gainSymbol !== "string" || !initial.gainSymbol)) return undefined;
      if (["pid-integral", "weighted-error"].includes(String(initial.source)) && (initial.weight !== undefined && !finite(initial.weight)
        || initial.weightSymbol !== undefined && (typeof initial.weightSymbol !== "string" || !initial.weightSymbol)
        || initial.weight !== undefined && initial.weightSymbol !== undefined)) return undefined;
      if (initial.source === "notch-history" && (!Array.isArray(initial.denominatorSymbols) || initial.denominatorSymbols.length !== 2
        || !initial.denominatorSymbols.every(symbol => typeof symbol === "string" && !!symbol))) return undefined;
    }
    names.add(state.symbol);
  }
  return structuredClone(value) as unknown as ControllerInitialization;
}
export function initializationValues(loop: ControlLoopDefinition, model: GeneralModel, definition: ControllerInitialization, parameters: Readonly<Record<string, number>> = {}): Record<string, number> {
  return Object.fromEntries(definition.states.map(state => {
    const initial = state.initial;
    let value: number;
    if (typeof initial === "number") value = initial;
    else if (["pid-integral", "weighted-error", "notch-history"].includes(initial.source)) {
      if (initial.source === "pid-integral" || initial.source === "weighted-error" || initial.source === "notch-history") {
        const output = model.outputs[initial.index];
        if (!output) throw new Error(`Initialization output for ${state.symbol} is unavailable.`);
        const parameter = (symbol: string) => {
          const binding = loop.parameterBinding?.parameters.find(parameter => parameter.symbol === symbol);
          const value = parameters[symbol];
          if (!binding || !Number.isFinite(value)) throw new Error(`Initialization parameter ${symbol} is not available.`);
          return value;
        };
        const process = model.operatingPoint?.outputs[initial.index] ?? 0;
        if (initial.source === "pid-integral") {
          if (state.unit !== model.inputs[initial.inputIndex]?.unit) throw new Error("Integral initialization must use the actuator coordinate unit.");
          const weight = initial.weightSymbol ? parameter(initial.weightSymbol) : initial.weight ?? 1;
          value = (model.operatingPoint?.inputs[initial.inputIndex] ?? 0) - parameter(initial.gainSymbol) * (weight - 1) * process;
        } else {
          if (state.unit !== output.unit) throw new Error("Filter/error history must use the process coordinate unit.");
          value = initial.source === "weighted-error" ? ((initial.weightSymbol ? parameter(initial.weightSymbol) : initial.weight ?? 0) - 1) * process
            : process / (1 + parameter(initial.denominatorSymbols[0]) + parameter(initial.denominatorSymbols[1]));
        }
      } else throw new Error("Unknown controller initialization formula.");
    }
    else if (initial.source === "scaled-model-input") {
      // A feedforward state K_ff·d at the operating point (Specification 35 §17.12).
      const input = model.inputs[initial.index], gain = loop.parameterBinding?.parameters.find(parameter => parameter.symbol === initial.gainSymbol);
      if (!input || !gain || gain.unit !== `${state.unit}/${input.unit}` || !Number.isFinite(parameters[initial.gainSymbol])) {
        throw new Error(`Initialization of ${state.symbol} needs the bound gain ${initial.gainSymbol} in ${state.unit}/${input?.unit ?? "input unit"}.`);
      }
      value = parameters[initial.gainSymbol] * (model.operatingPoint?.inputs[initial.index] ?? 0);
    }
    else if (initial.source === "decoupled-model-input") {
      value = decoupledInput(loop, model, initial.index, initial.matrixSymbols, parameters, state.unit, state.symbol);
    }
    else if (initial.source === "model-input-is") {
      const input = connectedModelChannels(loop, model).inputs[initial.index];
      if (!input || initial.value === undefined || !Number.isFinite(initial.value)
        || initial.value < input.signal.range.minimum || initial.value > input.signal.range.maximum || state.unit !== "1") {
        throw new Error(`Boolean operating-point initialization is incompatible with ${state.symbol}.`);
      }
      value = Number((model.operatingPoint?.inputs[initial.index] ?? 0) === initial.value);
    } else {
      const channels = initial.source === "model-input" ? model.inputs : initial.source === "model-output" ? model.outputs
        : model.kind === "transfer-function" ? [] : model.states;
      const index = initial.source === "model-input" || initial.source === "model-output"
        ? controllerModelIndex(loop, model, initial.source, initial.index) : initial.index;
      if (!channels[index] || channels[index].unit !== state.unit) throw new Error(`Initialization coordinates/units do not match ${state.symbol}.`);
      value = initial.source === "model-input" ? model.operatingPoint?.inputs[index] ?? 0
        : initial.source === "model-output" ? model.operatingPoint?.outputs[index] ?? 0
        : loop.channels?.initialState?.[index] ?? 0;
    }
    return [state.symbol, roundedTransferValue(state, value)];
  }));
}
/** Row `row` of D⁻¹·u at the model's operating inputs, in the group's declared input order (Specification 35 §17.15). */
function decoupledInput(loop: ControlLoopDefinition, model: GeneralModel, row: number, matrixSymbols: readonly (readonly string[])[],
  parameters: Readonly<Record<string, number>>, unit: string, symbol: string): number {
  const inputs = matrixSymbols.map((_, index) => controllerModelIndex(loop, model, "model-input", index));
  if (inputs.some(index => !model.inputs[index]) || model.inputs[inputs[row]].unit !== unit) throw new Error(`Initialization coordinates/units do not match ${symbol}.`);
  const matrix = matrixSymbols.map(entries => entries.map(entry => {
    if (!Number.isFinite(parameters[entry])) throw new Error(`Initialization parameter ${entry} is not available.`);
    return parameters[entry];
  }));
  return solveLinear(matrix, inputs.map(index => model.operatingPoint?.inputs[index] ?? 0))[row];
}

/** Solves A·x = b by Gaussian elimination with partial pivoting; a singular A is refused. */
export function solveLinear(matrix: readonly (readonly number[])[], right: readonly number[]): number[] {
  const size = right.length, a = matrix.map((entries, index) => [...entries, right[index]]);
  for (let column = 0; column < size; column++) {
    let pivot = column;
    for (let row = column + 1; row < size; row++) if (Math.abs(a[row][column]) > Math.abs(a[pivot][column])) pivot = row;
    if (!(Math.abs(a[pivot][column]) > 1e-12)) throw new Error("The decoupler matrix is singular.");
    [a[column], a[pivot]] = [a[pivot], a[column]];
    for (let row = column + 1; row < size; row++) {
      const factor = a[row][column] / a[column][column];
      for (let k = column; k <= size; k++) a[row][k] -= factor * a[column][k];
    }
  }
  const result = Array<number>(size).fill(0);
  for (let row = size - 1; row >= 0; row--) {
    let sum = a[row][size];
    for (let k = row + 1; k < size; k++) sum -= a[row][k] * result[k];
    result[row] = sum / a[row][row];
  }
  return result;
}
/** Group state declarations retain their channel identities when a model orders channels differently. */
export function controllerModelIndex(loop: ControlLoopDefinition, model: GeneralModel,
  source: "model-input" | "model-output", index: number): number {
  if (!loop.controller.group) return index;
  const declared = source === "model-input" ? loop.channels?.inputs : loop.channels?.outputs;
  const channels = source === "model-input" ? model.inputs : model.outputs;
  return channels.findIndex(channel => channel.id === declared?.[index]?.modelChannelId);
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function finite(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value); }
