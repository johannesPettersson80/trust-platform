import { assessCommissioningObservation, type CommissioningObservation } from "./commissioningObservation";
import { decodeRunRequirement, evaluateObservedRequirement, type RunRequirement } from "./runValidation";
import type { LoopRecording } from "./loopMonitor";

export interface CommissioningCriteria {
  target: number; durationSeconds: number; settlingTimeSeconds: number; maximumError: number;
  taskPeriodMs: number; processRange: { minimum: number; maximum: number };
  outputLimits: { minimum: number; maximum: number };
  observed?: CommissioningObservation;
  switching?: { low: number; high: number; minimumOnSeconds: number; minimumOffSeconds: number };
}

/** The same recorded-data assessment is used at completion and when reopening evidence. */
export function assessCommissioningRecording(recording: LoopRecording, criteria: CommissioningCriteria): number {
  const samples = recording.rawSamples ?? [];
  if (recording.evidence.completeness !== "complete" || samples.some(sample => sample.quality !== "good")) throw new Error("Commissioning recording is incomplete or has invalid samples.");
  const first = samples.findIndex(sample => sample.controllerMode === "Auto" && sample.sp === criteria.target);
  if (first < 0) throw new Error("The recorded controller never adopted the reviewed Auto target.");
  const start = samples[first].elapsedSeconds;
  const response = samples.slice(first);
  if (response[response.length - 1].elapsedSeconds - start < criteria.durationSeconds - criteria.taskPeriodMs / 1000) throw new Error("The recorded response does not cover the reviewed duration.");
  if (response.some(sample => sample.controllerMode !== "Auto" || sample.sp !== criteria.target || sample.pv === null || sample.appliedMv === null
    || sample.pv < criteria.processRange.minimum || sample.pv > criteria.processRange.maximum
    || sample.appliedMv < criteria.outputLimits.minimum || sample.appliedMv > criteria.outputLimits.maximum)) throw new Error("The commissioned loop violated mode, reference or signal bounds.");
  const settled = response.filter(sample => sample.elapsedSeconds - start >= criteria.settlingTimeSeconds);
  if (!settled.length) throw new Error("The response deadline was not recorded.");
  const error = Math.max(...settled.map(sample => Math.abs(criteria.target - sample.pv!)));
  if (error > criteria.maximumError) throw new Error(`Commissioning target error ${error} exceeds ${criteria.maximumError}.`);
  if (criteria.observed) assessCommissioningObservation(response, criteria.observed);
  if (criteria.switching) {
    const switching = criteria.switching;
    const observed = { channels: [{ id: "MV", unit: samples[first].units.mv }],
      conditions: { sampleTimesSeconds: response.map(sample => sample.elapsedSeconds - start) },
      samples: response.map(sample => ({ timeSeconds: sample.elapsedSeconds - start, sequence: sample.scan,
        values: { MV: { value: sample.appliedMv, quality: sample.quality } } })) };
    const window = { start: 0, end: observed.conditions.sampleTimesSeconds[observed.conditions.sampleTimesSeconds.length - 1] };
    const requirements: RunRequirement[] = [
      { id: "switching-output-levels", kind: "levels", channel: "MV", window, allowed: [switching.low, switching.high], tolerance: 0 },
      { id: "switching-on-dwell", kind: "dwell", channel: "MV", window, threshold: (switching.low + switching.high) / 2,
        state: "high", minimumSeconds: switching.minimumOnSeconds },
      { id: "switching-off-dwell", kind: "dwell", channel: "MV", window, threshold: (switching.low + switching.high) / 2,
        state: "low", minimumSeconds: switching.minimumOffSeconds },
    ];
    for (const requirement of requirements) {
      const result = evaluateObservedRequirement(observed, decodeRunRequirement(requirement, observed));
      if (result.status !== "Pass") throw new Error(`Commissioning ${result.id}: ${result.reason}`);
    }
  }
  return error;
}
