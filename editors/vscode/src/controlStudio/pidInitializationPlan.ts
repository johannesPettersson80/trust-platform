import { pidOperatingState } from "./pidOperatingState";
import { noiseAwareTransferTolerance } from "./proportionalTransfer";
import { decodePidLaw } from "./controllerLaw";
import { engineerVerificationPlan, type EngineerRequirements } from "./engineerVerificationPlan";
import type { GeneralModel } from "./generalModel";
import type { ControlLoopDefinition } from "./loopDefinition";
import { pidInitializationAdapter } from "./pidInitialization";

/** Builds the isolated actual-ST proof for a declared parallel PID Auto-entry state. */
export function pidInitializationPlan(loop: ControlLoopDefinition, model: GeneralModel,
  task: EngineerRequirements, values: Readonly<Record<string, number>>, processNoise?: number) {
  const adapter = pidInitializationAdapter(loop);
  const law = decodePidLaw(loop.controller.law, loop.parameterBinding);
  const binding = loop.parameterBinding;
  const reference = loop.signals.setpoint;
  const manual = loop.controller.manualOutputSignal;
  const manualType = loop.controller.manualOutputType;
  if (!law || !binding || !reference || !manual || !manualType || manualType === "BOOL") {
    throw new Error("The declared PI/PID initialization adapter is incompatible with the compiled reference, mode or Manual command.");
  }
  const proportionalGain = values[law.proportionalGainSymbol];
  if (!Number.isFinite(proportionalGain)) throw new Error("The candidate proportional gain is unavailable for PID state initialization.");
  const appliedOutput = model.operatingPoint?.inputs[0] ?? 0;
  const processValue = model.operatingPoint?.outputs[0] ?? 0;
  const history = pidOperatingState(loop, model, values);
  const initialIntegral = history[adapter.integralState.symbol];
  const initialDerivative = history[adapter.derivativeState.symbol];
  const initialManual = rounded({ iecType: manualType, minimum: loop.limits.outputMinimum, maximum: loop.limits.outputMaximum }, appliedOutput);
  const base = engineerVerificationPlan(loop, model, task, values);
  const step = base.request.step_ms;
  const duration = step * 6;
  const transferAt = step * 3;
  const window = { start: 0, end: duration / 1000 };
  const tolerance = noiseAwareTransferTolerance(loop, task.transferTolerance, proportionalGain, processNoise);
  const observations = uniqueObservations([...base.request.observations,
    { symbol: binding.mode.symbol, unit: "1" }, { symbol: manual, unit: loop.signals.manipulatedVariable.unit },
    { symbol: adapter.integralState.symbol, unit: adapter.integralState.unit },
    { symbol: adapter.derivativeState.symbol, unit: adapter.derivativeState.unit }]);
  return { integralState: initialIntegral, derivativeState: initialDerivative, reference: processValue, tolerance,
    request: { ...base.request, duration_ms: duration, max_total_cycles: 7,
      initial_values: { ...base.request.initial_values, [binding.mode.symbol]: Number(binding.mode.manual_value),
        [reference.symbol]: processValue, [manual]: initialManual,
        [adapter.integralState.symbol]: initialIntegral, [adapter.derivativeState.symbol]: initialDerivative },
      observations,
      events: [{ at_ms: transferAt, symbol: reference.symbol, value: processValue },
        { at_ms: transferAt, symbol: binding.mode.symbol, value: Number(!binding.mode.manual_value) }],
    }, requirements: [
      { id: "transfer-continuity", kind: "bounds" as const, channel: loop.signals.manipulatedVariable.symbol, window,
        minimum: appliedOutput - tolerance, maximum: appliedOutput + tolerance },
      { id: "auto-mode", kind: "bounds" as const, channel: binding.mode.symbol,
        window: { start: transferAt / 1000, end: window.end }, minimum: Number(!binding.mode.manual_value), maximum: Number(!binding.mode.manual_value) },
      { id: "integral-state", kind: "bounds" as const, channel: adapter.integralState.symbol, window,
        minimum: initialIntegral, maximum: initialIntegral },
      { id: "derivative-state", kind: "bounds" as const, channel: adapter.derivativeState.symbol, window,
        minimum: initialDerivative, maximum: initialDerivative },
      ...base.requirements.filter(requirement => requirement.kind === "bounds"),
    ].map(requirement => ({ ...requirement, window: requirement.id === "auto-mode" ? requirement.window : window })) };
}

function rounded(state: { iecType: "REAL" | "LREAL"; minimum: number; maximum: number }, value: number): number {
  const result = state.iecType === "REAL" ? Math.fround(value) : value;
  if (!Number.isFinite(result) || result < state.minimum || result > state.maximum) {
    throw new Error("The declared PI/PID initialization state is outside its IEC range after rounding.");
  }
  return result;
}

function uniqueObservations(values: readonly { symbol: string; unit: string }[]): { symbol: string; unit: string }[] {
  return [...new Map(values.map(value => [value.symbol, value])).values()];
}
