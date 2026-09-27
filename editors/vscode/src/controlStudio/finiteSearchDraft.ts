import type { ControlLoopDefinition, ControlSignalBinding } from "./loopDefinition";

/** Uses declared engineering meaning; the editor shows every initial value before Start. */
export function buildFiniteSearchDraft(loop: ControlLoopDefinition, values: Readonly<Record<string, number>>): Record<string, unknown> {
  const binding = loop.parameterBinding;
  const setpoint = loop.signals.setpoint;
  if (!binding || !setpoint) throw new Error("Declare tunable parameters and a reference signal to generate a search draft.");
  const step = loop.controller.taskPeriodMs;
  if (!Number.isInteger(step) || step < 1 || step > 1000) throw new Error("This search adapter supports whole-millisecond steps from 1 to 1000 ms.");
  const duration = Math.ceil((loop.experiment?.durationSeconds ?? 10) * 1000 / step) * step;
  const initial: Record<string, number> = { [binding.mode.symbol]: Number(!binding.mode.manual_value) };
  const grid = binding.parameters.slice(0, 4).map(parameter => {
    const name = Object.entries(loop.analysisParameterSymbols ?? {}).find(([, symbol]) => symbol === parameter.symbol)?.[0];
    const selected = name && loop.analysis?.rootLocus.parameter === name ? loop.analysis.rootLocus.values : [values[parameter.symbol]];
    const candidates = selected.map(value => parameter.iec_type === "REAL" ? Math.fround(value) : value);
    if (candidates.some(value => !Number.isFinite(value) || value < parameter.minimum || value > parameter.maximum)) throw new Error(`Inspect finite, bounded values for ${parameter.symbol} before preparing a search.`);
    return { symbol: parameter.symbol, unit: parameter.unit, minimum: parameter.minimum, maximum: parameter.maximum, values: candidates };
  });
  for (const parameter of binding.parameters.slice(4)) {
    const value = values[parameter.symbol];
    if (!Number.isFinite(value) || value < parameter.minimum || value > parameter.maximum) throw new Error(`Inspect a bounded fixed value for ${parameter.symbol}.`);
    initial[parameter.symbol] = value;
  }
  const signals = [setpoint, loop.signals.processVariable, loop.signals.manipulatedVariable,
    ...(loop.signals.actuator ? [loop.signals.actuator] : []), ...(loop.signals.additional ?? [])];
  const unique = [...new Map(signals.map(signal => [signal.symbol, signal])).values()];
  const unit = (signal: ControlSignalBinding) => {
    if (signal.unit) return signal.unit;
    if (signal.iecType.toUpperCase() === "BOOL") return "1";
    throw new Error(`Declare the engineering unit for ${signal.symbol}.`);
  };
  const actuator = loop.signals.actuator ?? loop.signals.manipulatedVariable;
  const process = loop.signals.processVariable;
  return { grid, trial_count: grid.reduce((count, axis) => count * axis.values.length, 1),
    step_ms: step, duration_ms: duration, seed: loop.experiment?.seed ?? 1,
    max_total_cycles: 50000, wall_budget_ms: 60000, per_trial_wall_ms: 10000,
    initial_values: initial, events: [], observations: unique.map(signal => ({ symbol: signal.symbol, unit: unit(signal) })),
    requirements: [
      { id: "process-bounds", symbol: process.symbol, minimum: process.range.minimum, maximum: process.range.maximum, start_ms: 0, end_ms: duration },
      { id: "actuator-bounds", symbol: actuator.symbol, minimum: loop.limits.outputMinimum, maximum: loop.limits.outputMaximum, start_ms: 0, end_ms: duration },
    ], objective: { reference_symbol: setpoint.symbol, feedback_symbol: process.symbol, error_unit: unit(process) } };
}
