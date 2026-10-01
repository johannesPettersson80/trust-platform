export interface PeriodicRecording { input: number[]; output: number[]; sampleTimeSeconds: number; recordingId: string; recordingRevision: string }

/** A retained recording usable for periodic measurement: complete, consecutive, unclipped, good-quality scans with one
 * cycle time. The first scan of a live recording has no predecessor, so its cycle time may be unknown. */
export function decodePeriodicRecording(payload: Record<string, unknown>): PeriodicRecording | undefined {
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
      return typeof sample.scan !== "number" || typeof previous.scan !== "number" || sample.scan !== previous.scan + 1 || sample.loopRevision !== usable[0].loopRevision;
    })) return undefined;
  const timed = usable.filter((sample, index) => !(index === 0 && sample.actualCycleSeconds === null));
  const sampleTimeSeconds = typeof timed[0]?.actualCycleSeconds === "number" && Number.isFinite(timed[0].actualCycleSeconds) ? timed[0].actualCycleSeconds : undefined;
  if (!sampleTimeSeconds || !Number.isFinite(sampleTimeSeconds) || sampleTimeSeconds <= 0) return undefined;
  if (timed.some(sample => typeof sample.actualCycleSeconds !== "number" || !Number.isFinite(sample.actualCycleSeconds)
      || Math.abs(sample.actualCycleSeconds - sampleTimeSeconds) > 1e-6 * Math.max(1, sampleTimeSeconds))) return undefined;
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
