import type { LoopRecording } from "./loopMonitor";

export interface RetainedPeriodicRecordingSource {
  input: readonly number[];
  output: readonly number[];
  sampleTimeSeconds: number;
  recordingId: string;
  recordingRevision: string;
}

/** Returns only complete, consecutive, uniformly sampled runtime evidence. */
export function retainedPeriodicRecordingSource(
  recording: LoopRecording | undefined,
  recordingId: string,
  recordingRevision: string,
): RetainedPeriodicRecordingSource | undefined {
  const raw = recording?.rawSamples;
  const evidence = recording?.evidence;
  if (!raw || raw.length < 32 || raw.length > 12000 || evidence?.completeness !== "complete"
    || evidence.recordingId !== recordingId || evidence.missingScanRanges.length !== 0
    || evidence.droppedObservedSamples !== 0 || !evidence.actualCycleTimingAvailable) return undefined;
  const provenanceRevision = recording?.provenance?.resultRevision === undefined
    ? raw[0]?.loopRevision
    : String(recording.provenance.resultRevision);
  if (provenanceRevision !== recordingRevision) return undefined;
  if (raw.some((sample, index) => sample.quality !== "good" || sample.appliedMv === null || sample.pv === null
      || !Number.isFinite(sample.appliedMv) || !Number.isFinite(sample.pv)
      || sample.appliedMv === sample.outputLimits.minimum || sample.appliedMv === sample.outputLimits.maximum
      || index > 0 && (sample.scan !== raw[index - 1].scan + 1 || sample.loopRevision !== raw[0].loopRevision))) return undefined;
  // The first scan of a live recording has no predecessor, so its cycle time may be unknown.
  const timed = raw[0].actualCycleSeconds === null ? raw.slice(1) : raw;
  const sampleTimeSeconds = timed[0]?.actualCycleSeconds ?? null;
  if (sampleTimeSeconds === null || !Number.isFinite(sampleTimeSeconds) || sampleTimeSeconds <= 0
    || timed.some(sample => sample.actualCycleSeconds === null || !Number.isFinite(sample.actualCycleSeconds)
      || Math.abs(sample.actualCycleSeconds - sampleTimeSeconds) > 1e-6 * Math.max(1, sampleTimeSeconds))) return undefined;
  return { input: raw.map(sample => sample.appliedMv as number), output: raw.map(sample => sample.pv as number), sampleTimeSeconds, recordingId, recordingRevision };
}
