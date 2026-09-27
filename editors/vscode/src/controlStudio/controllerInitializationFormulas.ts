import type { ControlLoopDefinition } from "./loopDefinition";
import type { GeneralModel } from "./generalModel";
import { controllerModelIndex, type ControllerInitialization } from "./controllerInitialState";
import { connectedModelChannels } from "./loopChannels";

export type InitializationFormula =
  | { kind: "pid-integral"; output_symbol: string; process_variable: string; gain_symbol: string; weight_symbol?: string; weight?: number }
  | { kind: "weighted-error"; process_variable: string; weight_symbol?: string; weight?: number }
  | { kind: "notch-history"; process_variable: string; denominator_symbols: readonly [string, string] }
  | { kind: "model-input"; output_symbol: string }
  | { kind: "model-output"; process_variable: string }
  | { kind: "model-input-is"; output_symbol: string; value: number };
export function initializationFormulas(loop: ControlLoopDefinition, model: GeneralModel,
  definition: ControllerInitialization): Readonly<Record<string, InitializationFormula>> | undefined {
  const channels = connectedModelChannels(loop, model), formulas: Record<string, InitializationFormula> = {};
  for (const state of definition.states) {
    const initial = state.initial;
    if (typeof initial === "number") continue;
    if (initial.source === "model-input" || initial.source === "model-input-is") {
      const input = channels.inputs[controllerModelIndex(loop, model, "model-input", initial.index)], output_symbol = input?.signal.symbol;
      if (!input || !output_symbol) throw new Error("The initialization formula has no bound actuator.");
      if (initial.source === "model-input-is") {
        if (initial.value === undefined || !Number.isFinite(initial.value)
          || initial.value < input.signal.range.minimum || initial.value > input.signal.range.maximum) {
          throw new Error("Boolean input-state initialization predicate must be a declared actuator level.");
        }
        formulas[state.symbol] = { kind: initial.source, output_symbol, value: initial.value };
      } else formulas[state.symbol] = { kind: initial.source, output_symbol };
      continue;
    }
    if (initial.source === "model-output") {
      const process_variable = channels.outputs[controllerModelIndex(loop, model, "model-output", initial.index)]?.signal.symbol;
      if (!process_variable) throw new Error("The initialization formula has no bound process measurement.");
      formulas[state.symbol] = { kind: initial.source, process_variable };
      continue;
    }
    if (initial.source === "model-state") continue;
    const process_variable = channels.outputs[initial.index]?.signal.symbol;
    if (!process_variable) throw new Error("The initialization formula has no bound process measurement.");
    if (initial.source === "pid-integral") {
      const output_symbol = channels.inputs[initial.inputIndex]?.signal.symbol;
      if (!output_symbol) throw new Error("The initialization formula has no bound actuator.");
      formulas[state.symbol] = { kind: initial.source, output_symbol, process_variable, gain_symbol: initial.gainSymbol,
        ...(initial.weightSymbol ? { weight_symbol: initial.weightSymbol } : {}), ...(initial.weight !== undefined ? { weight: initial.weight } : {}) };
    } else if (initial.source === "weighted-error") formulas[state.symbol] = { kind: initial.source, process_variable,
      ...(initial.weightSymbol ? { weight_symbol: initial.weightSymbol } : {}), ...(initial.weight !== undefined ? { weight: initial.weight } : {}) };
    else if (initial.source === "notch-history") formulas[state.symbol] = { kind: initial.source, process_variable, denominator_symbols: initial.denominatorSymbols };
  }
  return Object.keys(formulas).length ? formulas : undefined;
}

export function evaluateInitializationFormula(binding: import("./proportionalTransfer").TransferBinding,
  formula: InitializationFormula, initial: Readonly<Record<string, number | boolean>>): number {
  const value = (symbol: string) => {
    const number = Number(initial[symbol]);
    if (!Number.isFinite(number)) throw new Error(`Initialization snapshot is missing ${symbol}.`);
    return number;
  };
  const freshOutput = (symbol: string) => value(binding.tracking_outputs?.[symbol] ?? symbol);
  if (formula.kind === "model-input") return freshOutput(formula.output_symbol);
  if (formula.kind === "model-input-is") return Number(freshOutput(formula.output_symbol) === formula.value);
  if (formula.kind === "model-output") return value(formula.process_variable);
  if (formula.kind === "pid-integral") return value(binding.tracking_outputs?.[formula.output_symbol] ?? formula.output_symbol)
    - value(formula.gain_symbol) * ((formula.weight_symbol ? value(formula.weight_symbol) : formula.weight ?? 1) - 1) * value(formula.process_variable);
  if (formula.kind === "weighted-error") return ((formula.weight_symbol ? value(formula.weight_symbol) : formula.weight ?? 0) - 1) * value(formula.process_variable);
  return value(formula.process_variable) / (1 + value(formula.denominator_symbols[0]) + value(formula.denominator_symbols[1]));
}
