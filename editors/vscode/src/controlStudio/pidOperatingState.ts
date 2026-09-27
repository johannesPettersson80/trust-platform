import type { ControlLoopDefinition } from "./loopDefinition";
import type { GeneralModel } from "./generalModel";
import { decodePidLaw } from "./controllerLaw";
import { roundedTransferValue } from "./proportionalTransfer";

/** Retained parallel-PID history at the selected operating point, before the target change. */
export function pidOperatingState(loop: ControlLoopDefinition, model: GeneralModel, values: Readonly<Record<string, number>>): Record<string, number> {
  const adapter = loop.controller.pidInitialization, law = decodePidLaw(loop.controller.law, loop.parameterBinding);
  if (!adapter || adapter.kind !== "parallel-pid-v1" || !law) return {};
  const process = model.operatingPoint?.outputs[0] ?? 0, output = model.operatingPoint?.inputs[0] ?? 0;
  const gain = values[law.proportionalGainSymbol];
  if (!Number.isFinite(gain)) throw new Error("A finite PID gain is required to initialize operating-point history.");
  const state = (member: typeof adapter.integralState, value: number) => roundedTransferValue({
    symbol: member.symbol, iec_type: member.iecType, unit: member.unit, minimum: member.minimum, maximum: member.maximum,
  }, value);
  return {
    [adapter.integralState.symbol]: state(adapter.integralState, output - gain * (adapter.proportionalSetpointWeight - 1) * process),
    [adapter.derivativeState.symbol]: state(adapter.derivativeState, (adapter.derivativeSetpointWeight - 1) * process),
  };
}
