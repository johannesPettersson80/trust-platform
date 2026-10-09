import { firstGap, gapPhrase } from "../evenSampling";
import { steadyRecordPeriod } from "../recordSequence";

export interface PeriodicRecording { input: number[]; output: number[]; sampleTimeSeconds: number; recordingId: string; recordingRevision: string }

/** A retained recording usable for periodic measurement: complete, consecutive, unclipped, good-quality scans whose cycle times
 * jitter normally. The first scan of a live recording has no predecessor, so its cycle time may be unknown. A cycle longer than one
 * and a half median cycles is a gap that refuses the recording; `onRefusal`, when given, receives the gap's reason. */
export function decodePeriodicRecording(payload: Record<string, unknown>, onRefusal?: (reason: string) => void): PeriodicRecording | undefined {
  const recording = isRecord(payload.recording) ? payload.recording : payload;
  if (!Array.isArray(recording.rawSamples)) return undefined;
  const usable = recording.rawSamples.filter(isRecord);
  const evidence = isRecord(recording.evidence) ? recording.evidence : isRecord(recording.acquisition) ? recording.acquisition : undefined;
  if (usable.length < 32 || evidence?.completeness !== "complete" || !Array.isArray(evidence.missingScanRanges) || evidence.missingScanRanges.length !== 0
    || evidence.droppedObservedSamples !== 0 || evidence.actualCycleTimingAvailable !== true
    || usable.some((sample, index) => {
      if (sample.quality !== "good" || typeof sample.appliedMv !== "number" || !Number.isFinite(sample.appliedMv) || typeof sample.pv !== "number" || !Number.isFinite(sample.pv)
        || (isRecord(sample.outputLimits) && (sample.appliedMv === sample.outputLimits.minimum || sample.appliedMv === sample.outputLimits.maximum))) return true;
      if (index === 0) return false;
      const previous = usable[index - 1];
      return typeof sample.scan !== "number" || typeof previous.scan !== "number" || sample.loopRevision !== usable[0].loopRevision;
    })) return undefined;
  if (!steadyRecordPeriod(usable.map(sample => sample.scan as number), true)) return undefined;
  // Each later scan's cycle time is the spacing from the scan before it; the first scan's own cycle lies before the recording.
  const cycles = usable.slice(1).map(sample => sample.actualCycleSeconds);
  if (cycles.some(cycle => typeof cycle !== "number" || !Number.isFinite(cycle) || cycle <= 0)) return undefined;
  const spacings = cycles as number[];
  // The grid step is the median spacing. Task jitter is normal; a spacing longer than one and a half steps is a gap.
  const sampleTimeSeconds = median(spacings);
  const times = [0];
  for (const spacing of spacings) times.push(times[times.length - 1] + spacing);
  const gap = firstGap(times, sampleTimeSeconds);
  if (gap) { onRefusal?.(`The recording has ${gapPhrase(gap)}.`); return undefined; }
  const provenance = isRecord(recording.provenance) ? recording.provenance : undefined;
  const recordingId = typeof evidence?.recordingId === "string" ? evidence.recordingId : undefined;
  const recordingRevision = typeof provenance?.resultRevision === "number" ? String(provenance.resultRevision) : typeof usable[0].loopRevision === "string" ? usable[0].loopRevision : undefined;
  if (!recordingId || !recordingRevision) return undefined;
  return { input: usable.map(sample => sample.appliedMv as number), output: usable.map(sample => sample.pv as number), sampleTimeSeconds, recordingId, recordingRevision };
}

/** The recording of a project experiment that completed, which the analyses may measure; nothing for other phases. */
export function completedExperimentRecording(message: { kind?: unknown; phase?: unknown; payload?: unknown }): Record<string, unknown> | undefined {
  if (message.kind !== "control-studio.experiment.result" || message.phase !== "terminal" || !isRecord(message.payload)
    || message.payload.status !== "completed" || !isRecord(message.payload.recording) || !Array.isArray(message.payload.recording.rawSamples)) return undefined;
  return message.payload.recording;
}

function isRecord(value: unknown): value is Record<string, unknown> { return Boolean(value) && typeof value === "object" && !Array.isArray(value); }

function median(values: readonly number[]): number {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.floor(sorted.length / 2)];
}
