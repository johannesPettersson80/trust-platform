import type { DeclaredControlAnalysis } from "./declaredAnalysis";
/** Wire declaration shared with control.parameters.*; runtime verifies compiled ownership. */
export interface ParameterBinding {
  loop_id: string;
  task_name: string;
  parameters: readonly {
    symbol: string;
    iec_type: "REAL" | "LREAL";
    unit: string;
    convention: string;
    minimum: number;
    maximum: number;
  }[];
  mode: { symbol: string; manual_value: boolean };
}

export function decodeParameterBinding(value: unknown, loopId: unknown): ParameterBinding | undefined {
  if (!record(value) || !keys(value, ["loop_id", "task_name", "parameters", "mode"]) ||
      !label(value.loop_id) || value.loop_id !== loopId || !label(value.task_name) ||
      !record(value.mode) || !keys(value.mode, ["symbol", "manual_value"]) ||
      !label(value.mode.symbol) || typeof value.mode.manual_value !== "boolean" ||
      !Array.isArray(value.parameters) || value.parameters.length === 0 || value.parameters.length > 64) return undefined;
  const symbols = new Set<string>([value.mode.symbol]);
  for (const parameter of value.parameters) {
    if (!record(parameter) || !keys(parameter, ["symbol", "iec_type", "unit", "convention", "minimum", "maximum"]) ||
        !label(parameter.symbol) || symbols.has(parameter.symbol) ||
        !["REAL", "LREAL"].includes(String(parameter.iec_type)) || !label(parameter.unit) || !label(parameter.convention) ||
        !finite(parameter.minimum) || !finite(parameter.maximum) || parameter.minimum > parameter.maximum) return undefined;
    symbols.add(parameter.symbol);
  }
  return value as unknown as ParameterBinding;
}

function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function keys(value: Record<string, unknown>, expected: string[]): boolean { return Object.keys(value).length === expected.length && expected.every(key => Object.prototype.hasOwnProperty.call(value, key)); }
function label(value: unknown): value is string { return typeof value === "string" && value.trim().length > 0 && value.length <= 1024 && !/[\u0000-\u001f]/.test(value); }
function finite(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value); }

/** Analysis names are explicit; a symbol's spelling alone never selects a live gain. */
export function validAnalysisParameterSymbols(value: unknown, binding: ParameterBinding | undefined, analysis: DeclaredControlAnalysis | undefined): value is Record<string, string> {
  if (!record(value) || !binding || !analysis || Object.keys(value).length !== Object.keys(analysis.controller.parameters).length) return false;
  const used = new Set<string>();
  return Object.entries(analysis.controller.parameters).every(([name, declared]) => {
    const symbol = value[name];
    const parameter = binding.parameters.find(entry => entry.symbol === symbol);
    if (typeof symbol !== "string" || used.has(symbol) || !parameter || parameter.unit !== declared.unit ||
        parameter.minimum > declared.minimum || parameter.maximum < declared.maximum) return false;
    used.add(symbol);
    return true;
  });
}

/** Materializes one declared coefficient set without losing unrelated observed fields. */
export function materializeBoundParameters(binding: ParameterBinding, expectedSymbols: readonly string[], proposed: Readonly<Record<string, number>>,
  observed: Readonly<Record<string, number>>): Record<string, number> {
  const expected = new Set(expectedSymbols);
  if (expected.size !== expectedSymbols.length || Object.keys(proposed).length !== expected.size || Object.keys(proposed).some(symbol => !expected.has(symbol))) {
    throw new Error("A complete, unique declared parameter record is required.");
  }
  const values = { ...observed };
  for (const symbol of expected) {
    const parameter = binding.parameters.find(parameter => parameter.symbol === symbol);
    const value = proposed[symbol];
    if (!parameter || !Number.isFinite(value) || value < parameter.minimum || value > parameter.maximum
      || parameter.iec_type === "REAL" && Math.fround(value) !== value) throw new Error(`Parameter ${symbol} is outside its representable binding.`);
    values[symbol] = value;
  }
  return values;
}
