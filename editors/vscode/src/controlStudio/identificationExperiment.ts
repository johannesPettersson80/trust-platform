import type { ControlLoopDefinition } from "./loopDefinition";
import { loopSignal } from "./loopChannels";
import type { IdentificationChannels } from "./identifiedStateModel";
import type { IdentificationSample } from "./identification";
import type { RecordedRun } from "./runValidation";

export interface ExperimentTrace {
  id: string;
  unit: string;
  points: readonly { timeSeconds: number; value: number | null }[];
}
export interface IdentificationExperiment {
  recordingId?: string;
  inputs: readonly ExperimentTrace[];
  outputs: readonly ExperimentTrace[];
}

/** Display the recorded coordinates; never generate an ideal excitation or resimulate. */
export function experimentFromRun(loop: ControlLoopDefinition, run: RecordedRun, selected?: IdentificationChannels): IdentificationExperiment {
  const signals = (kind: "inputs" | "outputs") => selected?.[kind] ??
    (loop.channels?.[kind].map(channel => loopSignal(loop, channel.signalId)) ??
      [kind === "inputs" ? loop.signals.manipulatedVariable : loop.signals.processVariable])
      .map(signal => ({ id: signal.symbol, unit: signal.unit }));
  const traces = (kind: "inputs" | "outputs") => signals(kind).map(signal => {
    if (!run.channels.some(channel => channel.id === signal.id && channel.unit === signal.unit)) {
      throw new Error("The experiment recording does not contain its selected signal and unit.");
    }
    return { ...signal, points: run.samples.map(sample => {
      const reading = sample.values[signal.id];
      return { timeSeconds: sample.timeSeconds, value: reading?.quality === "good" && Number.isFinite(reading.value) ? reading.value : null };
    }) };
  });
  return { recordingId: run.id, inputs: traces("inputs"), outputs: traces("outputs") };
}

export function experimentFromSamples(loop: ControlLoopDefinition, samples: readonly IdentificationSample[]): IdentificationExperiment {
  const trace = (key: "input" | "output", signal: { symbol: string; unit: string }): ExperimentTrace => ({
    id: signal.symbol, unit: signal.unit, points: samples.map(sample => ({ timeSeconds: sample.timeSeconds,
      value: sample.quality === "good" && Number.isFinite(sample[key]) ? sample[key] : null })),
  });
  return { inputs: [trace("input", loop.signals.manipulatedVariable)], outputs: [trace("output", loop.signals.processVariable)] };
}

export function decodeIdentificationExperiment(value: unknown): IdentificationExperiment | undefined {
  if (!record(value) || value.recordingId !== undefined && (typeof value.recordingId !== "string" || !value.recordingId)) return undefined;
  for (const kind of ["inputs", "outputs"]) {
    const traces = value[kind];
    if (!Array.isArray(traces) || !traces.length || traces.length > 32 || new Set(traces.map(trace => trace?.id)).size !== traces.length) return undefined;
    for (const trace of traces) {
      if (!record(trace) || typeof trace.id !== "string" || !trace.id || typeof trace.unit !== "string" || !trace.unit
        || !Array.isArray(trace.points) || trace.points.length > 20000) return undefined;
      let previous = -Infinity;
      for (const point of trace.points) {
        if (!record(point) || !finite(point.timeSeconds) || point.timeSeconds < 0 || point.timeSeconds <= previous
          || point.value !== null && !finite(point.value)) return undefined;
        previous = point.timeSeconds;
      }
    }
  }
  return value as unknown as IdentificationExperiment;
}
function record(value: unknown): value is Record<string, any> { return !!value && typeof value === "object" && !Array.isArray(value); }
function finite(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value); }
