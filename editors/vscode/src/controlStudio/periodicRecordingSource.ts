import type { LoopRecording } from "./loopMonitor";
import { blockAveraged, firstGap, MAXIMUM_FIT_SAMPLES, medianSpacing, resampledEvenly } from "./evenSampling";
import { steadyRecordPeriod } from "./recordSequence";

export interface RetainedPeriodicRecordingSource {
  input: readonly number[];
  output: readonly number[];
  sampleTimeSeconds: number;
  recordingId: string;
  recordingRevision: string;
}

/** Returns only complete, consecutive runtime evidence, placed on its even grid: the command is held between scans and the measured
 * output is interpolated. Task jitter is normal; a cycle longer than one and a half median cycles is a gap and is refused. */
export function retainedPeriodicRecordingSource(
  recording: LoopRecording | undefined,
  recordingId: string,
  recordingRevision: string,
): RetainedPeriodicRecordingSource | undefined {
  const raw = recording?.rawSamples;
  const evidence = recording?.evidence;
  if (!raw || raw.length < 32 || evidence?.completeness !== "complete"
    || evidence.recordingId !== recordingId || evidence.missingScanRanges.length !== 0
    || evidence.droppedObservedSamples !== 0 || !evidence.actualCycleTimingAvailable) return undefined;
  const provenanceRevision = recording?.provenance?.resultRevision === undefined
    ? raw[0]?.loopRevision
    : String(recording.provenance.resultRevision);
  if (provenanceRevision !== recordingRevision) return undefined;
  if (raw.some((sample, index) => sample.quality !== "good" || sample.appliedMv === null || sample.pv === null
      || !Number.isFinite(sample.appliedMv) || !Number.isFinite(sample.pv)
      || sample.appliedMv === sample.outputLimits.minimum || sample.appliedMv === sample.outputLimits.maximum
      || index > 0 && sample.loopRevision !== raw[0].loopRevision)
    || !steadyRecordPeriod(raw.map(sample => sample.scan), true)) return undefined;
  // The first scan of a live recording has no predecessor, so its cycle time may be unknown; each later cycle places its scan in time.
  const cycles = raw.slice(1).map(sample => sample.actualCycleSeconds);
  if (cycles.some(cycle => cycle === null || !Number.isFinite(cycle) || cycle <= 0)) return undefined;
  const cycleSeconds = cycles as number[];
  const sampleTimeSeconds = median(cycleSeconds);
  const times = [0];
  for (const cycle of cycleSeconds) times.push(times[times.length - 1] + cycle);
  if (firstGap(times, sampleTimeSeconds)) return undefined;
  const scans = raw.map((sample, index) => ({ timeSeconds: times[index], input: sample.appliedMv as number,
    output: sample.pv as number, quality: "good" as const }));
  const even = resampledEvenly(scans, sampleTimeSeconds);
  const fitted = blockAveraged(even, MAXIMUM_FIT_SAMPLES);
  return { input: fitted.map(sample => sample.input), output: fitted.map(sample => sample.output),
    sampleTimeSeconds: fitted.length === even.length ? sampleTimeSeconds : medianSpacing(fitted.map(sample => sample.timeSeconds)),
    recordingId, recordingRevision };
}

function median(values: readonly number[]): number {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.floor(sorted.length / 2)];
}
