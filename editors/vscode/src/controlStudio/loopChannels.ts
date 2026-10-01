import type { ControlLoopDefinition, ControlSignalBinding } from "./loopDefinition";
import type { GeneralModel } from "./generalModel";

export interface ActuatorCapabilities {
  kind: "continuous" | "on-off";
  minimum: number;
  maximum: number;
  rateUpPerS?: number;
  rateDownPerS?: number;
  minimumOnSeconds?: number;
  minimumOffSeconds?: number;
}
export interface ChannelRequirements {
  referenceChange: number;
  maximumError: number;
  settlingBand: number;
  settlingTimeSeconds: number;
}
export interface LoopChannels {
  /** `experimentStep` is this input's own identification step in its unit (Specification 35 §17.9). */
  inputs: readonly { modelChannelId: string; signalId: string; manualOutputSignal?: string; manualOutputType?: "REAL" | "LREAL" | "BOOL"; appliedSignalId?: string; actuator: ActuatorCapabilities; experimentStep?: number }[];
  outputs: readonly { modelChannelId: string; signalId: string; setpointSignalId?: string; requirements?: ChannelRequirements }[];
  /** State deviations in the selected model's declared coordinates. */
  initialState?: readonly number[];
}
export function validLoopChannels(value: unknown): value is LoopChannels {
  if (!record(value) || !Array.isArray(value.inputs) || !Array.isArray(value.outputs)) return false;
  const channels = (items: unknown[]) => items.length > 0 && items.length <= 16 && items.every(item => record(item)
    && text(item.modelChannelId) && text(item.signalId))
    && new Set(items.map(item => (item as Record<string, unknown>).modelChannelId)).size === items.length
    && new Set(items.map(item => (item as Record<string, unknown>).signalId)).size === items.length;
  return channels(value.inputs) && channels(value.outputs)
    && value.inputs.every(item => validActuator(item.actuator) && (item.manualOutputSignal === undefined || text(item.manualOutputSignal))
      && (item.manualOutputType === undefined || item.manualOutputType === "REAL" || item.manualOutputType === "LREAL" || item.manualOutputType === "BOOL") && (item.appliedSignalId === undefined || text(item.appliedSignalId))
      && (item.experimentStep === undefined || finite(item.experimentStep) && item.experimentStep > 0
        && item.experimentStep <= (item.actuator as { maximum: number }).maximum - (item.actuator as { minimum: number }).minimum))
    && value.outputs.every(item => (item.setpointSignalId === undefined || text(item.setpointSignalId))
      && (item.requirements === undefined || validChannelRequirements(item.requirements)))
    && (value.initialState === undefined || Array.isArray(value.initialState) && value.initialState.length <= 32 && value.initialState.every(finite));
}
export function validChannelRequirements(value: unknown): value is ChannelRequirements {
  return record(value) && finite(value.referenceChange) && finite(value.maximumError) && value.maximumError > 0
    && finite(value.settlingBand) && value.settlingBand > 0 && value.settlingBand < value.maximumError
    && finite(value.settlingTimeSeconds) && value.settlingTimeSeconds > 0;
}
export function validActuator(value: unknown): value is ActuatorCapabilities {
  return record(value) && ["continuous", "on-off"].includes(String(value.kind))
    && (value.kind !== "on-off" || value.rateUpPerS === undefined && value.rateDownPerS === undefined)
    && finite(value.minimum) && finite(value.maximum) && value.minimum < value.maximum
    && ["rateUpPerS", "rateDownPerS", "minimumOnSeconds", "minimumOffSeconds"].every(key =>
      value[key] === undefined || finite(value[key]) && value[key] >= 0);
}
/** A measured disturbance is a model input the controller only reads (Specification 35 §17.12). */
export function measuredDisturbanceInput(loop: ControlLoopDefinition, input: { signalId: string }): boolean {
  return loop.signals.additional?.some(signal => signal.id === input.signalId && signal.role === "disturbance") ?? false;
}
export function loopSignal(loop: ControlLoopDefinition, id: string): ControlSignalBinding {
  const primary = id === "setpoint" ? loop.signals.setpoint : id === "processVariable" ? loop.signals.processVariable
    : id === "manipulatedVariable" ? loop.signals.manipulatedVariable : id === "actuator" ? loop.signals.actuator : undefined;
  const signal = primary ?? loop.signals.additional?.find(signal => signal.id === id);
  if (!signal) throw new Error(`Channel binding ${id} does not identify a declared signal.`);
  return signal;
}
export function validateLoopChannelBindings(loop: ControlLoopDefinition): void {
  if (!loop.channels) return;
  if (!validLoopChannels(loop.channels)) throw new Error("Invalid multichannel binding, actuator or requirement.");
  const role = (id: string) => id === "manipulatedVariable" ? "MV" : id === "processVariable" ? "PV" : id === "setpoint" ? "SP"
    : loop.signals.additional?.find(signal => signal.id === id)?.role;
  // A measured disturbance is a model input the controller never writes (Specification 35 §17.12).
  if (loop.channels.inputs.some((input, index) => role(input.signalId) !== "MV" && !(index > 0 && role(input.signalId) === "disturbance"))
    || loop.channels.outputs.some(output => !["PV", "state"].includes(role(output.signalId) ?? "")
      || output.setpointSignalId !== undefined && role(output.setpointSignalId) !== "SP")) {
    throw new Error("Model input, output and setpoint bindings must match their declared signal roles.");
  }
  const claimed = new Set<string>();
  for (const channel of [...loop.channels.inputs, ...loop.channels.outputs]) {
    const signal = loopSignal(loop, channel.signalId);
    if (claimed.has(signal.symbol)) throw new Error(`Signal ${signal.symbol} is assigned to multiple model channels.`);
    claimed.add(signal.symbol);
  }
  for (const input of loop.channels.inputs) {
    const signal = loopSignal(loop, input.signalId);
    if (signal.iecType === "BOOL" && (signal.unit !== "1" || input.actuator.kind !== "on-off" || input.actuator.minimum !== 0 || input.actuator.maximum !== 1)) throw new Error("Boolean actuation requires ON/OFF levels 0/1 in dimensionless coordinates.");
    const applied = input.appliedSignalId ? loopSignal(loop, input.appliedSignalId) : signal.symbol === loop.signals.manipulatedVariable.symbol ? loop.signals.actuator : undefined;
    if (applied && (applied.unit !== signal.unit || applied.symbol === signal.symbol || !applied.address?.startsWith("%I"))) throw new Error("Applied actuator feedback requires a separate input binding in command coordinates.");
    if (input.actuator.minimum < signal.range.minimum || input.actuator.maximum > signal.range.maximum) {
      throw new Error(`Actuator limits exceed ${signal.symbol}'s declared range.`);
    }
  }
  for (const output of loop.channels.outputs) {
    if (output.requirements && !output.setpointSignalId) throw new Error(`Controlled output ${output.modelChannelId} needs a setpoint.`);
    if (output.setpointSignalId && loopSignal(loop, output.setpointSignalId).unit !== loopSignal(loop, output.signalId).unit) {
      throw new Error(`Setpoint units disagree for ${output.modelChannelId}.`);
    }
  }
}

/** Resolve by explicit model identity, never by display order or matching units alone. */
export function connectedModelChannels(loop: ControlLoopDefinition, model: GeneralModel) {
  validateLoopChannelBindings(loop);
  const configured = loop.channels;
  if (!configured) {
    if (model.inputs.length !== 1 || model.outputs.length !== 1) throw new Error("Map every model input and output in loop channel bindings.");
    return {
      inputs: [{ signal: loop.signals.manipulatedVariable, applied: loop.signals.actuator, actuator: { kind: loop.controller.onOff ? "on-off" as const : "continuous" as const,
        minimum: loop.controller.onOff?.lowOutput ?? loop.limits.outputMinimum, maximum: loop.controller.onOff?.highOutput ?? loop.limits.outputMaximum,
        rateUpPerS: loop.limits.rateUpPerS, rateDownPerS: loop.limits.rateDownPerS }, disturbance: false }],
      outputs: [{ signal: loop.signals.processVariable, reference: loop.signals.setpoint, requirements: undefined as ChannelRequirements | undefined }],
      initialState: undefined as readonly number[] | undefined,
    };
  }
  if (configured.inputs.length !== model.inputs.length || configured.outputs.length !== model.outputs.length) {
    throw new Error("Every selected-model channel must have exactly one binding.");
  }
  return {
    inputs: model.inputs.map(channel => {
      const input = configured.inputs.find(input => input.modelChannelId === channel.id);
      if (!input) throw new Error(`Missing input binding for ${channel.id}.`);
      return { signal: loopSignal(loop, input.signalId), actuator: input.actuator, disturbance: measuredDisturbanceInput(loop, input),
        applied: input.appliedSignalId ? loopSignal(loop, input.appliedSignalId) : input.signalId === "manipulatedVariable" ? loop.signals.actuator : undefined };
    }),
    outputs: model.outputs.map(channel => {
      const output = configured.outputs.find(output => output.modelChannelId === channel.id);
      if (!output) throw new Error(`Missing output binding for ${channel.id}.`);
      return { signal: loopSignal(loop, output.signalId), reference: output.setpointSignalId ? loopSignal(loop, output.setpointSignalId) : undefined,
        requirements: output.requirements };
    }),
    initialState: configured.initialState,
  };
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function finite(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value); }
function text(value: unknown): value is string { return typeof value === "string" && value.trim().length > 0; }
