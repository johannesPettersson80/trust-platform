import type { ControlLoopDefinition } from "./loopDefinition";
import { loopSignal } from "./loopChannels";

export interface AcquisitionSettings {
  method: "configured" | "automatic" | "mv_step" | "mv_pulse" | "setpoint_step" | "relay" | "prbs" | "multisine";
  durationSeconds: number;
  amplitude: number;
  setpointSignalId?: string;
  relayDirection?: "positive" | "negative";
  reuseRecordings: boolean;
  inputRange?: { minimum: number; maximum: number };
  outputRange?: { minimum: number; maximum: number };
  inputRanges?: Readonly<Record<string, { minimum: number; maximum: number }>>;
  outputRanges?: Readonly<Record<string, { minimum: number; maximum: number }>>;
}
export function acquisitionSettings(value: unknown): AcquisitionSettings | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
  const settings = value as AcquisitionSettings;
  if (!["configured", "automatic", "mv_step", "mv_pulse", "setpoint_step", "relay", "prbs", "multisine"].includes(settings.method)
    || settings.setpointSignalId !== undefined && (typeof settings.setpointSignalId !== "string" || !settings.setpointSignalId || settings.setpointSignalId.length > 256)
    || !Number.isFinite(settings.durationSeconds) || settings.durationSeconds < 2 || settings.durationSeconds > 600
    || !Number.isFinite(settings.amplitude) || typeof settings.reuseRecordings !== "boolean"
    || settings.relayDirection !== undefined && settings.relayDirection !== "positive" && settings.relayDirection !== "negative"
    || settings.method === "relay" && !settings.relayDirection
    || settings.inputRange !== undefined && !validRange(settings.inputRange)
    || settings.outputRange !== undefined && !validRange(settings.outputRange)
    || !validRangeMap(settings.inputRanges) || !validRangeMap(settings.outputRanges)) return undefined;
  return structuredClone(settings);
}
export function configuredAcquisition(loop: ControlLoopDefinition, settings?: AcquisitionSettings): ControlLoopDefinition {
  if (!settings) {
    if (!Number.isFinite(loop.experiment?.durationSeconds) || loop.experiment!.durationSeconds <= 0) throw new AcquisitionLimitsRequired();
    return loop;
  }
  const reference = settings.setpointSignalId ? loop.signals.additional?.find(signal => signal.id === settings.setpointSignalId && signal.role === "SP") : undefined;
  if (settings.setpointSignalId && (!reference || reference.unit !== loop.signals.processVariable.unit
    || loop.signals.setpoint && reference.symbol !== loop.signals.setpoint.symbol)) throw new Error("Choose a declared setpoint signal in the measured output's units.");
  const input = loop.signals.manipulatedVariable, output = loop.signals.processVariable;
  const actuator = loop.channels?.inputs.find(channel => channel.signalId === "manipulatedVariable")?.actuator;
  const inputBounds = { minimum: Math.max(input.range.minimum, loop.limits.outputMinimum, actuator?.minimum ?? -Infinity),
    maximum: Math.min(input.range.maximum, loop.limits.outputMaximum, actuator?.maximum ?? Infinity) };
  const inputRanges = channelRanges(loop, settings.inputRanges, "input");
  const outputRanges = channelRanges(loop, settings.outputRanges, "output");
  const inputRange = boundedRange(inputRanges?.[input.symbol] ?? settings.inputRange, inputBounds, input.symbol);
  const outputRange = boundedRange(outputRanges?.[output.symbol] ?? settings.outputRange, output.range, output.symbol);
  const channelInputs = loop.channels?.inputs.map(channel => {
    const signal = loopSignal(loop, channel.signalId);
    if (channel.signalId === "manipulatedVariable") return { ...channel, actuator: { ...channel.actuator, ...inputRange } };
    const bounds = { minimum: Math.max(signal.range.minimum, channel.actuator.minimum), maximum: Math.min(signal.range.maximum, channel.actuator.maximum) };
    const range = boundedRange(inputRanges?.[signal.symbol], bounds, signal.symbol);
    return { ...channel, actuator: { ...channel.actuator, ...range } };
  });
  const additional = loop.signals.additional?.map(signal => {
    const range = signal.role === "MV" ? inputRanges?.[signal.symbol] : signal.role === "PV" || signal.role === "state" ? outputRanges?.[signal.symbol] : undefined;
    return range ? { ...signal, range: boundedRange(range, signal.range, signal.symbol) } : signal;
  });
  return { ...loop,
    ...(loop.channels ? { channels: { ...loop.channels, inputs: channelInputs ?? loop.channels.inputs.map(channel => channel.signalId === "manipulatedVariable"
      ? { ...channel, actuator: { ...channel.actuator, ...inputRange } } : channel) } } : {}),
    signals: { ...loop.signals, ...(reference ? { setpoint: reference } : {}), manipulatedVariable: { ...input, range: inputRange }, processVariable: { ...output, range: outputRange }, ...(additional ? { additional } : {}) },
    limits: { ...loop.limits, outputMinimum: inputRange.minimum, outputMaximum: inputRange.maximum },
    experiment: { execution: "control-studio", seed: 1, ...loop.experiment,
    kind: settings.method === "configured" ? loop.experiment?.kind ?? "observation" : settings.method === "automatic" ? "observation" : settings.method,
    durationSeconds: settings.durationSeconds, amplitude: settings.amplitude, relayDirection: settings.relayDirection ?? loop.experiment?.relayDirection } };
}

export class AcquisitionLimitsRequired extends Error {
  readonly missing = "limits";
  constructor() { super("Set the maximum experiment duration in Limits before identification."); }
}

function validRange(range: AcquisitionSettings["inputRange"]): boolean {
  return !!range && Number.isFinite(range.minimum) && Number.isFinite(range.maximum) && range.minimum < range.maximum;
}
function validRangeMap(ranges: AcquisitionSettings["inputRanges"]): boolean {
  return ranges === undefined || !!ranges && Object.keys(ranges).length <= 16 && Object.values(ranges).every(validRange);
}
function channelRanges(loop: ControlLoopDefinition, ranges: AcquisitionSettings["inputRanges"], direction: "input" | "output") {
  if (!ranges) return undefined;
  const signals = direction === "input"
    ? [loop.signals.manipulatedVariable, ...(loop.signals.additional ?? []).filter(signal => signal.role === "MV")]
    : [loop.signals.processVariable, ...(loop.signals.additional ?? []).filter(signal => signal.role === "PV" || signal.role === "state")];
  const allowed = new Set(signals.map(signal => signal.symbol));
  if (Object.keys(ranges).some(symbol => !allowed.has(symbol))) throw new Error(`Unknown ${direction} channel range.`);
  return ranges;
}
function boundedRange(requested: AcquisitionSettings["inputRange"], declared: NonNullable<AcquisitionSettings["inputRange"]>, signal: string) {
  const range = requested ?? declared;
  if (!validRange(range) || range.minimum < declared.minimum || range.maximum > declared.maximum) {
    throw new Error(`Experiment range for ${signal} must stay within its configured limits (${declared.minimum} to ${declared.maximum}).`);
  }
  return { ...range };
}
