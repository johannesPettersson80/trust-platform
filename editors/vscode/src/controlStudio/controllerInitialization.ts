import { initializationFormulas, type InitializationFormula } from "./controllerInitializationFormulas";
import { withActuatorFeedback, transferOutputBasis } from "./transferActuatorFeedback";
import { commissioningReferenceTargets } from "./commissioningObservation";
import { declaredControllerChannels } from "./declaredControllerChannels";
import { connectedModelChannels } from "./loopChannels";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { GeneralModel } from "./generalModel";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import { engineerVerificationPlan } from "./engineerVerificationPlan";
import { decodePidLaw } from "./controllerLaw";
import { noiseAwareTransferTolerance, roundedTransferValue, type TransferBinding, type TransferMember } from "./proportionalTransfer";
import type { TransferStatus } from "./runtimeTransfer";
import type { CommissioningProfile } from "./commissioningProfile";

import { initializationValues, type ControllerInitialization } from "./controllerInitialState";
export { decodeControllerInitialization, initializationValues, type ControllerInitialization } from "./controllerInitialState";

export interface DeclaredStateTracking {
  kind: "declared-state-v1";
  reference_symbol: string;
  manual_command_symbol: string;
  process_variable: string;
  align_output: boolean;
  initialization_formulas?: Readonly<Record<string, InitializationFormula>>;
  initialization_values: Readonly<Record<string, number>>;
  bounded_state_symbols: readonly string[];
  additional_references?: readonly { reference_symbol: string; process_variable: string }[];
  additional_measurements?: readonly string[];
  additional_commands?: readonly { manual_command_symbol: string; output_symbol: string }[];
}
export function declaredTransferBinding(loop: ControlLoopDefinition, definition: ControllerInitialization): TransferBinding {
  const parameters = loop.parameterBinding, reference = loop.signals.setpoint, manual = loop.controller.manualOutputSignal;
  if (!parameters || !reference || !manual || !loop.controller.manualOutputType) throw new Error("Declare the reference, Manual command and mode before controller initialization.");
  const channels = declaredControllerChannels(loop);
  const output = channels.outputs[0].output;
  const additional_outputs = channels.outputs.slice(1).map(channel => channel.output);
  const states = [channels.references[0].reference, channels.outputs[0].command,
    ...channels.references.slice(1).map(channel => channel.reference), ...channels.outputs.slice(1).map(channel => channel.command),
    ...definition.states.map(({ symbol, iec_type, unit, minimum, maximum }) => ({ symbol, iec_type, unit, minimum, maximum }))];
  const guards = [...parameters.parameters.map(({ symbol, iec_type, unit, minimum, maximum }) => ({ symbol, iec_type, unit, minimum, maximum })),
    ...channels.measurements];
  const additional_modes = (loop.controller.project?.additionalModes ?? []).map(mode => ({ symbol: mode.symbol, manual_value: mode.manualValue }));
  const symbols = [parameters.mode.symbol, ...additional_modes.map(mode => mode.symbol), ...states.map(state => state.symbol), ...guards.map(guard => guard.symbol),
    output.symbol, ...additional_outputs.map(output => output.symbol)];
  if (new Set(symbols.map(symbol => symbol.toUpperCase())).size !== symbols.length) throw new Error("Controller initialization roles overlap parameters, measurements or output ownership.");
  return withActuatorFeedback(loop, { loop_id: parameters.loop_id, task_name: parameters.task_name, mode: parameters.mode,
    ...(additional_modes.length ? { additional_modes } : {}), states, guards, output, ...(additional_outputs.length ? { additional_outputs } : {}) });
}
export function declaredCommissioningProfile(loop: ControlLoopDefinition, model: GeneralModel, definition: ControllerInitialization, label: string, parameters: Readonly<Record<string, number>> = {}): CommissioningProfile {
  const channels = declaredControllerChannels(loop);
  const formulas = initializationFormulas(loop, model, definition);
  return { label,
    references: () => channels.references.map(channel => channel.reference.symbol),
    targets: (model, task) => commissioningReferenceTargets(loop, model, task),
    requiredTracking: formulas ? ["declared-initialization-formulas"] : [],
    trackingKind: "declared-state-v1", binding: loop => declaredTransferBinding(loop, definition),
    states: (binding: TransferBinding, observed: TransferStatus) => Object.fromEntries(binding.states.map(state => {
      const value = observed.values[state.symbol];
      const numeric = typeof value === "boolean" ? Number(value) : value;
      if (typeof numeric !== "number") throw new Error(`Controller state ${state.symbol} is not available.`);
      return [state.symbol, roundedTransferValue(state, numeric)];
    })),
    tracking: (_loop, binding, initialize = true): DeclaredStateTracking => ({ kind: "declared-state-v1",
      reference_symbol: binding.states[0].symbol, manual_command_symbol: binding.states[1].symbol,
      process_variable: loop.signals.processVariable.symbol, align_output: initialize,
      additional_references: channels.references.slice(1).map(channel => ({ reference_symbol: channel.reference.symbol, process_variable: channel.process.symbol })),
      additional_measurements: channels.measurements.map(channel => channel.symbol),
      additional_commands: channels.outputs.slice(1).map(channel => ({ manual_command_symbol: channel.command.symbol, output_symbol: channel.output.symbol })),
      initialization_values: initialize ? initializationValues(loop, model, definition, parameters)
        : Object.fromEntries(definition.states.map(state => [state.symbol, roundedTransferValue(state, state.iec_type === "BOOL" ? 0 : state.minimum / 2 + state.maximum / 2)])),
      ...(formulas ? { initialization_formulas: formulas } : {}),
      bounded_state_symbols: definition.states.filter(state => state.postScan === "bounded").map(state => state.symbol) }),
  };
}
export function declaredInitializationPlan(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
  values: Readonly<Record<string, number>>, definition: ControllerInitialization, processNoise?: number) {
  const binding = declaredTransferBinding(loop, definition);
  const base = engineerVerificationPlan(loop, model, task, values);
  const initialized = Object.fromEntries(definition.states.map(state => [state.symbol, base.request.initial_values[state.symbol]]));
  const step = base.request.step_ms, duration = step * 6, transferAt = step * 3;
  const channels = declaredControllerChannels(loop), modelChannels = connectedModelChannels(loop, model);
  const referenceIndex = modelChannels.outputs.findIndex(channel => channel.signal.symbol === loop.signals.processVariable.symbol);
  const outputIndex = modelChannels.inputs.findIndex(channel => channel.signal.symbol === loop.signals.manipulatedVariable.symbol);
  const reference = model.operatingPoint?.outputs[referenceIndex] ?? 0;
  const initialValue = (signal: string, index: number) => {
    const planned = base.request.initial_values[signal];
    return typeof planned === "number" ? planned : model.operatingPoint?.inputs[index] ?? 0;
  };
  const output = initialValue(loop.signals.manipulatedVariable.symbol, outputIndex);
  const pid = decodePidLaw(loop.controller.law, loop.parameterBinding);
  // A program's own P, PI or PID names its gains in its seed; noise reaches its first output as it reaches Control.PID's.
  // A trajectory's feedforward is zero at rest, so its PID alone answers the noise (§17.16).
  const seed = loop.controller.project?.seed, own = seed?.kind === "parallel-pid" || seed?.kind === "trajectory-feedforward-pid" ? seed : undefined;
  const gains = pid ? { proportional: values[pid.proportionalGainSymbol], integral: values[pid.integralGainSymbol], derivative: values[pid.derivativeGainSymbol] }
    : own ? { proportional: values[own.proportionalSymbol], integral: own.integralSymbol ? values[own.integralSymbol] : undefined,
      derivative: own.derivativeSymbol ? values[own.derivativeSymbol] : undefined } : undefined;
  const tolerance = noiseAwareTransferTolerance(loop, task.transferTolerance, gains?.proportional, processNoise,
    gains ? { integralGain: gains.integral, derivativeGain: gains.derivative, cycleSeconds: loop.controller.taskPeriodMs / 1000 } : {});
  const window = { start: 0, end: duration / 1000 };
  const held: Record<string, number> = { [binding.states[0].symbol]: roundedTransferValue(binding.states[0], reference),
    [binding.states[1].symbol]: roundedTransferValue(binding.states[1], output) };
  for (const channel of channels.references) {
    const index = modelChannels.outputs.findIndex(output => output.signal.symbol === channel.process.symbol);
    held[channel.reference.symbol] = roundedTransferValue(channel.reference, model.operatingPoint?.outputs[index] ?? 0);
  }
  for (const channel of channels.outputs) {
    const index = modelChannels.inputs.findIndex(input => input.signal.symbol === channel.output.symbol);
    held[channel.command.symbol] = roundedTransferValue(channel.command, initialValue(channel.output.symbol, index));
  }
  const observations = [...new Map([...base.request.observations, ...binding.states.map(state => ({ symbol: state.symbol, unit: state.unit })),
    { symbol: binding.mode.symbol, unit: "1" }].map(signal => [signal.symbol, signal])).values()];
  return { reference, tolerance, request: { ...base.request, duration_ms: duration, max_total_cycles: 7, observations,
    initial_values: { ...base.request.initial_values, ...held, ...initialized, [binding.mode.symbol]: Number(binding.mode.manual_value) },
    events: [...Object.entries({ ...held, ...initialized }).map(([symbol, value]) => ({ at_ms: transferAt, symbol, value })),
      { at_ms: transferAt, symbol: binding.mode.symbol, value: Number(!binding.mode.manual_value) }],
  }, requirements: [
    { id: "transfer-continuity", kind: "bounds" as const, channel: binding.output.symbol, window, minimum: output - tolerance, maximum: output + tolerance },
    ...channels.outputs.slice(1).map(channel => {
      const value = held[channel.command.symbol];
      const primaryBasis = transferOutputBasis(binding, binding.output);
      const basis = transferOutputBasis(binding, channel.output);
      const span = primaryBasis.maximum - primaryBasis.minimum;
      const allowance = span > 0 ? tolerance * (basis.maximum - basis.minimum) / span : 0;
      return { id: `transfer-continuity:${channel.output.symbol}`, kind: "bounds" as const, channel: channel.output.symbol,
        window, minimum: value - allowance, maximum: value + allowance };
    }),
    { id: "auto-mode", kind: "bounds" as const, channel: binding.mode.symbol, window: { start: transferAt / 1000, end: window.end },
      minimum: Number(!binding.mode.manual_value), maximum: Number(!binding.mode.manual_value) },
    ...definition.states.map(state => ({ id: `initialized-state:${state.symbol}`, kind: "state-bounds" as const, channel: state.symbol,
      window: { start: transferAt / 1000, end: window.end },
      minimum: state.postScan === "unchanged" ? initialized[state.symbol] : state.minimum,
      maximum: state.postScan === "unchanged" ? initialized[state.symbol] : state.maximum })),
    ...base.requirements.filter(requirement => requirement.kind === "bounds").map(requirement => ({ ...requirement, window })),
  ] };
}
