import type { ControlLoopDefinition } from "../loopDefinition";
import type { AcquisitionSettings } from "../acquisitionSettings";
import { displayEngineeringUnit } from "../chartContract";
import { openCompactNumberEditor } from "../../webview/compactEditor";
import { loopSignal } from "../loopChannels";

export interface StudioExperimentLimits {
  durationSeconds?: number;
  inputMinimum: number;
  inputMaximum: number;
  outputMinimum: number;
  outputMaximum: number;
  inputRanges?: Record<string, { minimum: number; maximum: number }>;
  outputRanges?: Record<string, { minimum: number; maximum: number }>;
}

export function initialExperimentLimits(loop: ControlLoopDefinition): StudioExperimentLimits {
  const inputs = loop.channels?.inputs.map(channel => loopSignal(loop, channel.signalId)) ?? [loop.signals.manipulatedVariable];
  const outputs = loop.channels?.outputs.map(channel => loopSignal(loop, channel.signalId)) ?? [loop.signals.processVariable];
  return {
    durationSeconds: loop.experiment?.durationSeconds,
    inputMinimum: Math.max(loop.signals.manipulatedVariable.range.minimum, loop.limits.outputMinimum,
      loop.channels?.inputs.find(channel => channel.signalId === "manipulatedVariable")?.actuator.minimum ?? -Infinity),
    inputMaximum: Math.min(loop.signals.manipulatedVariable.range.maximum, loop.limits.outputMaximum,
      loop.channels?.inputs.find(channel => channel.signalId === "manipulatedVariable")?.actuator.maximum ?? Infinity),
    outputMinimum: loop.signals.processVariable.range.minimum,
    outputMaximum: loop.signals.processVariable.range.maximum,
    inputRanges: Object.fromEntries(inputs.map(signal => {
      const channel = loop.channels?.inputs.find(candidate => loopSignal(loop, candidate.signalId).symbol === signal.symbol);
      const minimum = Math.max(signal.range.minimum, channel?.actuator.minimum ?? -Infinity, signal.symbol === loop.signals.manipulatedVariable.symbol ? loop.limits.outputMinimum : -Infinity);
      const maximum = Math.min(signal.range.maximum, channel?.actuator.maximum ?? Infinity, signal.symbol === loop.signals.manipulatedVariable.symbol ? loop.limits.outputMaximum : Infinity);
      return [signal.symbol, { minimum, maximum }];
    })),
    outputRanges: Object.fromEntries(outputs.map(signal => [signal.symbol, { minimum: signal.range.minimum, maximum: signal.range.maximum }])),
  };
}

/** Edit a copy; only Done commits the complete checked set. */
export function editExperimentLimits(loop: ControlLoopDefinition, values: StudioExperimentLimits,
  commit: (limits: StudioExperimentLimits) => void, waiting = false, cancelled?: () => void): void {
  const input = loop.signals.manipulatedVariable, output = loop.signals.processVariable;
  const inputs = loop.channels?.inputs.map(channel => loopSignal(loop, channel.signalId)) ?? [input];
  const outputs = loop.channels?.outputs.map(channel => loopSignal(loop, channel.signalId)) ?? [output];
  const multichannel = inputs.length > 1 || outputs.length > 1;
  const inputRanges = multichannel ? values.inputRanges ?? { [input.symbol]: { minimum: values.inputMinimum, maximum: values.inputMaximum } } : { [input.symbol]: { minimum: values.inputMinimum, maximum: values.inputMaximum } };
  const outputRanges = multichannel ? values.outputRanges ?? { [output.symbol]: { minimum: values.outputMinimum, maximum: values.outputMaximum } } : { [output.symbol]: { minimum: values.outputMinimum, maximum: values.outputMaximum } };
  const fields = [
    ...inputs.flatMap(signal => rangeFields("input", signal, inputRanges[signal.symbol] ?? boundedInputRange(loop, signal), multichannel, loop)),
    ...outputs.flatMap(signal => rangeFields("output", signal, outputRanges[signal.symbol] ?? signal.range, multichannel, loop)),
    { key: "durationSeconds", label: "Maximum duration (s)", value: values.durationSeconds, minimum: 2, maximum: 600 },
  ];
  openCompactNumberEditor({ title: "Experiment limits", fieldAttribute: "data-studio-limit", cancelled,
    initialIssue: waiting && values.durationSeconds === undefined ? { field: "durationSeconds", message: "Enter the maximum experiment duration." } : undefined, fields: [
    ...fields,
  ], validate: next => validateRanges(next, inputs, outputs),
  commit: next => {
    const committed: StudioExperimentLimits = {
      ...values,
      durationSeconds: next.durationSeconds,
      inputMinimum: Number(next[fieldKey("inputMinimum", input.symbol, multichannel)]), inputMaximum: Number(next[fieldKey("inputMaximum", input.symbol, multichannel)]),
      outputMinimum: Number(next[fieldKey("outputMinimum", output.symbol, multichannel)]), outputMaximum: Number(next[fieldKey("outputMaximum", output.symbol, multichannel)]),
      ...(multichannel ? { inputRanges: Object.fromEntries(inputs.map(signal => [signal.symbol, { minimum: Number(next[fieldKey("inputMinimum", signal.symbol, true)]), maximum: Number(next[fieldKey("inputMaximum", signal.symbol, true)]) }])),
        outputRanges: Object.fromEntries(outputs.map(signal => [signal.symbol, { minimum: Number(next[fieldKey("outputMinimum", signal.symbol, true)]), maximum: Number(next[fieldKey("outputMaximum", signal.symbol, true)]) }])) } : {}),
    };
    commit(committed);
  } });
}

function rangeFields(kind: "input" | "output", signal: { symbol: string; unit: string; range: { minimum: number; maximum: number } }, range: { minimum: number; maximum: number }, multichannel: boolean, loop: ControlLoopDefinition) {
  const group = `${kind === "input" ? "Plant input" : "Measured output"} · ${signal.symbol} (${displayEngineeringUnit(signal.unit)})`;
  const minimum = kind === "input" ? boundedInputRange(loop, signal).minimum : signal.range.minimum;
  const maximum = kind === "input" ? boundedInputRange(loop, signal).maximum : signal.range.maximum;
  return [
    { key: fieldKey(`${kind}Minimum`, signal.symbol, multichannel), label: "Minimum", group, signal: signal.symbol, unit: signal.unit, value: range.minimum, minimum, maximum },
    { key: fieldKey(`${kind}Maximum`, signal.symbol, multichannel), label: "Maximum", group, signal: signal.symbol, unit: signal.unit, value: range.maximum, minimum, maximum },
  ];
}

function validateRanges(next: Readonly<Record<string, number>>, inputs: readonly { symbol: string }[], outputs: readonly { symbol: string }[]) {
  const multichannel = inputs.length > 1 || outputs.length > 1;
  for (const [kind, signals] of [["input", inputs], ["output", outputs]] as const) {
    for (const signal of signals) {
      const minimum = next[fieldKey(`${kind}Minimum`, signal.symbol, multichannel)], maximum = next[fieldKey(`${kind}Maximum`, signal.symbol, multichannel)];
      if (minimum >= maximum) return { field: fieldKey(`${kind}Maximum`, signal.symbol, multichannel), message: "Maximum must be greater than minimum." };
    }
  }
  return undefined;
}

function fieldKey(prefix: string, symbol: string, multichannel: boolean): string { return multichannel ? `${prefix}:${symbol}` : prefix; }
function boundedInputRange(loop: ControlLoopDefinition, signal: { symbol: string; range: { minimum: number; maximum: number } }) {
  const channel = loop.channels?.inputs.find(candidate => loopSignal(loop, candidate.signalId).symbol === signal.symbol);
  return { minimum: Math.max(signal.range.minimum, channel?.actuator.minimum ?? -Infinity, signal.symbol === loop.signals.manipulatedVariable.symbol ? loop.limits.outputMinimum : -Infinity),
    maximum: Math.min(signal.range.maximum, channel?.actuator.maximum ?? Infinity, signal.symbol === loop.signals.manipulatedVariable.symbol ? loop.limits.outputMaximum : Infinity) };
}

export function acquisitionPreferences(loop: ControlLoopDefinition, method: AcquisitionSettings["method"], limits: StudioExperimentLimits): AcquisitionSettings {
  if (limits.durationSeconds === undefined) throw new Error("Set the maximum experiment duration in Limits before identification.");
  return { method, durationSeconds: limits.durationSeconds, amplitude: loop.experiment?.amplitude ?? 0,
    inputRange: { minimum: limits.inputMinimum, maximum: limits.inputMaximum },
    outputRange: { minimum: limits.outputMinimum, maximum: limits.outputMaximum },
    ...(limits.inputRanges && Object.keys(limits.inputRanges).length > 1 ? { inputRanges: limits.inputRanges } : {}),
    ...(limits.outputRanges && Object.keys(limits.outputRanges).length > 1 ? { outputRanges: limits.outputRanges } : {}),
    reuseRecordings: false, ...(loop.experiment?.relayDirection ? { relayDirection: loop.experiment.relayDirection } : {}) };
}
