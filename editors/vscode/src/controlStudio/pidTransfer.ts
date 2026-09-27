import { withActuatorFeedback } from "./transferActuatorFeedback";
import { decodePidLaw } from "./controllerLaw";
import type { ControlLoopDefinition } from "./loopDefinition";
import { pidInitializationAdapter } from "./pidInitialization";
import { roundedTransferValue, type TransferBinding, type TransferMember } from "./proportionalTransfer";
import type { TransferStatus } from "./runtimeTransfer";

export interface ParallelPidTracking {
  kind: "parallel-pid-v1";
  reference_symbol: string;
  manual_command_symbol: string;
  process_variable: string;
  proportional_gain_symbol: string;
  integral_gain_symbol: string;
  derivative_gain_symbol: string;
  integral_state_symbol: string;
  derivative_state_symbol: string;
  proportional_setpoint_weight: number;
  derivative_setpoint_weight: number;
  cycle_seconds: number;
  align_output: boolean;
}

/** Creates a transfer boundary only from the explicit PID state contract. */
export function pidTransferBinding(loop: ControlLoopDefinition): TransferBinding {
  const adapter = pidInitializationAdapter(loop);
  const law = decodePidLaw(loop.controller.law, loop.parameterBinding);
  const parameters = loop.parameterBinding;
  const reference = loop.signals.setpoint;
  const manual = loop.controller.manualOutputSignal;
  const manualType = loop.controller.manualOutputType;
  if (!law || !parameters || !reference || !manual || !manualType || manualType === "BOOL") {
    throw new Error("The declared PI/PID initialization adapter is incompatible with the compiled parameter, reference, mode or Manual command binding.");
  }
  const member = (signal: { symbol: string; iecType: string; unit: string; range: { minimum: number; maximum: number } }): TransferMember => {
    if (signal.iecType !== "REAL" && signal.iecType !== "LREAL") throw new Error("PID Auto commissioning requires REAL or LREAL state bindings.");
    return { symbol: signal.symbol, iec_type: signal.iecType, unit: signal.unit, minimum: signal.range.minimum, maximum: signal.range.maximum };
  };
  const state = (value: { symbol: string; iecType: "REAL" | "LREAL"; unit: string; minimum: number; maximum: number }): TransferMember =>
    ({ symbol: value.symbol, iec_type: value.iecType, unit: value.unit, minimum: value.minimum, maximum: value.maximum });
  const output = { ...member(loop.signals.manipulatedVariable), minimum: loop.limits.outputMinimum, maximum: loop.limits.outputMaximum };
  return withActuatorFeedback(loop, { loop_id: parameters.loop_id, task_name: parameters.task_name, mode: parameters.mode,
    states: [member(reference), { ...output, symbol: manual, iec_type: manualType }, state(adapter.integralState), state(adapter.derivativeState)],
    guards: [...parameters.parameters.map(({ symbol, iec_type, unit, minimum, maximum }) => ({ symbol, iec_type, unit, minimum, maximum })), member(loop.signals.processVariable)], output });
}

export function parallelPidTracking(loop: ControlLoopDefinition, binding: TransferBinding, alignOutput = true): ParallelPidTracking {
  const adapter = pidInitializationAdapter(loop);
  const law = decodePidLaw(loop.controller.law, loop.parameterBinding);
  if (!law) throw new Error("The declared PID law is unavailable for Auto initialization.");
  return { kind: "parallel-pid-v1", reference_symbol: binding.states[0].symbol, manual_command_symbol: binding.states[1].symbol,
    process_variable: loop.signals.processVariable.symbol, proportional_gain_symbol: law.proportionalGainSymbol,
    integral_gain_symbol: law.integralGainSymbol, derivative_gain_symbol: law.derivativeGainSymbol, integral_state_symbol: adapter.integralState.symbol,
    derivative_state_symbol: adapter.derivativeState.symbol, proportional_setpoint_weight: adapter.proportionalSetpointWeight,
    derivative_setpoint_weight: adapter.derivativeSetpointWeight, cycle_seconds: loop.controller.taskPeriodMs / 1000, align_output: alignOutput };
}

/** Carries only exact observed states into a reviewed transaction; no state is guessed. */
export function pidTransferStates(binding: TransferBinding, observed: TransferStatus): Record<string, number> {
  const values: Record<string, number> = {};
  for (const member of binding.states) {
    const value = observed.values[member.symbol];
    if (typeof value !== "number") throw new Error(`The runtime did not return the declared PID state ${member.symbol}.`);
    values[member.symbol] = roundedTransferValue(member, value);
  }
  return values;
}
