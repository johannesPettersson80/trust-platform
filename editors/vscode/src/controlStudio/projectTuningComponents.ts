import type { ControlLoopDefinition } from "./loopDefinition";
import type { ParameterBinding } from "./parameterBinding";

export type ProjectTuningComponent =
  | { kind: "low-pass"; timeConstantSymbol: string; timeConstantsSeconds: readonly number[]; order: 1 | 2 }
  | { kind: "notch"; convention: "direct-form-minus-feedback"; symbols: readonly [string, string, string, string, string]; frequenciesHz: readonly number[]; qualityFactors: readonly number[] }
  | { kind: "reference-weighting"; proportionalSymbol: string; derivativeSymbol: string; proportionalWeights: readonly number[]; derivativeWeights: readonly number[] }
  | { kind: "back-calculation"; gainSymbol: string; trackingTimesSeconds: readonly number[] };
export interface ComponentChoice { values: Record<string, number>; complexity: number }
export function componentSymbols(component: ProjectTuningComponent): readonly string[] {
  return component.kind === "low-pass" ? [component.timeConstantSymbol] : component.kind === "notch" ? component.symbols
    : component.kind === "reference-weighting" ? [component.proportionalSymbol, component.derivativeSymbol] : [component.gainSymbol];
}
export function validProjectComponents(value: unknown, binding: ParameterBinding): value is readonly ProjectTuningComponent[] {
  if (!Array.isArray(value) || value.length > 8) return false;
  const symbols = new Set<string>();
  try {
    for (const component of value) {
      if (!component || typeof component !== "object" || !["low-pass", "notch", "reference-weighting", "back-calculation"].includes(component.kind)) return false;
      for (const symbol of componentSymbols(component)) {
        if (typeof symbol !== "string" || symbols.has(symbol) || !binding.parameters.some(parameter => parameter.symbol === symbol)) return false;
        symbols.add(symbol);
      }
      const grid = (values: unknown, valid: (value: number) => boolean) => Array.isArray(values) && values.length > 0 && values.length <= 16 && values.every(value => typeof value === "number" && Number.isFinite(value) && valid(value));
      if (component.kind === "low-pass" && (![1, 2].includes(component.order) || !grid(component.timeConstantsSeconds, value => value >= 0))) return false;
      if (component.kind === "notch" && (component.convention !== "direct-form-minus-feedback" || component.symbols.length !== 5 || !grid(component.frequenciesHz, value => value > 0) || !grid(component.qualityFactors, value => value >= 0.5))) return false;
      if (component.kind === "reference-weighting" && (!grid(component.proportionalWeights, value => value >= 0 && value <= 1) || !grid(component.derivativeWeights, value => value >= 0 && value <= 1))) return false;
      if (component.kind === "back-calculation" && !grid(component.trackingTimesSeconds, value => value > 0)) return false;
      const unit = component.kind === "low-pass" ? "s" : component.kind === "back-calculation" ? "1/s" : "1";
      if (componentSymbols(component).some(symbol => binding.parameters.find(parameter => parameter.symbol === symbol)!.unit !== unit)) return false;
    }
    return true;
  } catch { return false; }
}

/** Candidate coefficients use explicit ST conventions; actual ST includes their full connected dynamics. */
export function componentChoices(loop: ControlLoopDefinition, component: ProjectTuningComponent, pins: Readonly<Record<string, number>>): ComponentChoice[] {
  const dt = loop.controller.taskPeriodMs / 1000;
  let choices: ComponentChoice[];
  const choice = (values: readonly number[], complexity: number) => ({ values: Object.fromEntries(componentSymbols(component).map((symbol, index) => [symbol, values[index]])), complexity });
  if (component.kind === "low-pass") choices = [...new Set([0, ...component.timeConstantsSeconds])].map(time => choice([time], time === 0 ? 0 : component.order));
  else if (component.kind === "reference-weighting") choices = component.proportionalWeights.flatMap(beta => component.derivativeWeights.map(gamma => choice([beta, gamma], beta === 1 && gamma === 0 ? 0 : 1)));
  else if (component.kind === "back-calculation") choices = component.trackingTimesSeconds.map(time => choice([1 / Math.max(dt, time)], 1));
  else {
    choices = [choice([1, 0, 0, 0, 0], 0)];
    for (const frequency of component.frequenciesHz) for (const quality of component.qualityFactors) {
      const omega = 2 * Math.PI * frequency * dt;
      if (omega >= Math.PI || omega <= 0) throw new Error("Notch frequency must lie below the actual sampled Nyquist limit.");
      const alpha = Math.sin(omega) / (2 * quality), denominator = 1 + alpha;
      choices.push(choice([1 / denominator, -2 * Math.cos(omega) / denominator, 1 / denominator,
        -2 * Math.cos(omega) / denominator, (1 - alpha) / denominator], 2));
    }
  }
  const realized: ComponentChoice[] = [];
  for (const candidate of choices) {
    let valid = true;
    for (const symbol of Object.keys(candidate.values)) {
      const parameter = loop.parameterBinding!.parameters.find(parameter => parameter.symbol === symbol)!;
      const value = symbol in pins ? pins[symbol] : candidate.values[symbol];
      const rounded = parameter.iec_type === "REAL" ? Math.fround(value) : value;
      if (!Number.isFinite(rounded) || rounded < parameter.minimum || rounded > parameter.maximum || symbol in pins && value !== rounded) { valid = false; break; }
      candidate.values[symbol] = rounded;
    }
    if (valid && component.kind === "low-pass") {
      const time = candidate.values[component.timeConstantSymbol]; valid = time >= 0; candidate.complexity = time === 0 ? 0 : component.order;
    }
    if (valid && component.kind === "reference-weighting") {
      const beta = candidate.values[component.proportionalSymbol], gamma = candidate.values[component.derivativeSymbol];
      valid = beta >= 0 && beta <= 1 && gamma >= 0 && gamma <= 1; candidate.complexity = beta === 1 && gamma === 0 ? 0 : 1;
    }
    if (valid && component.kind === "back-calculation") {
      valid = candidate.values[component.gainSymbol] >= 0; candidate.complexity = candidate.values[component.gainSymbol] === 0 ? 0 : 1;
    }
    if (valid && component.kind === "notch") {
      candidate.complexity = component.symbols.every((symbol, index) => candidate.values[symbol] === Number(index === 0)) ? 0 : 2;
      const [, , , a1, a2] = component.symbols.map(symbol => candidate.values[symbol]);
      valid = 1 + a1 + a2 > 1e-12 && 1 - a1 + a2 > 1e-12 && 1 - a2 > 1e-12;
    }
    if (valid && !realized.some(previous => JSON.stringify(previous.values) === JSON.stringify(candidate.values))) realized.push(candidate);
  }
  if (!realized.length) throw new Error("No representable component candidate satisfies its pinned settings, bounds and filter stability restrictions.");
  return realized;
}
