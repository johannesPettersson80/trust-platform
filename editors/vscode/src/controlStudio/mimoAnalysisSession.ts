/** V7.17 debt: a parked loop's retained-recording MIMO analysis on the TypeScript route. A served loop's measured paths are the engine's (C7). */
import { createHash } from "crypto";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { LoopRecording } from "./loopMonitor";
import { calculateRecordedMimoAnalysis, type MimoSettledAnalysisOptions, type MimoSettledTrial, type RecordedMimoAnalysis } from "./threePageMimoAnalysis";

export interface MimoAnalysisSnapshot {
  recordingId: string;
  recordingHash: string;
  trials: readonly MimoSettledTrial[];
  options?: MimoSettledAnalysisOptions;
  result: RecordedMimoAnalysis;
}

/** Owns selected-recording identity and settled-window evidence for MIMO analysis. */
export class MimoAnalysisSession {
  private snapshotValue: MimoAnalysisSnapshot | undefined;

  analyze(loop: ControlLoopDefinition, recording: LoopRecording, recordingId: string, trials: readonly MimoSettledTrial[], options?: MimoSettledAnalysisOptions): MimoAnalysisSnapshot {
    if (!recording.rawSamples?.length || !recording.evidence.recordingId || recording.evidence.recordingId !== recordingId) {
      throw new Error("Select a complete retained recording before calculating MIMO paths.");
    }
    const result = calculateRecordedMimoAnalysis(loop, recording.rawSamples, trials, options);
    if (result.status === "rejected") throw new Error(result.reason);
    this.snapshotValue = { recordingId, recordingHash: recordingHash(recording), trials: structuredClone(trials), options: options ? structuredClone(options) : undefined, result: structuredClone(result) };
    return structuredClone(this.snapshotValue);
  }

  snapshot(): MimoAnalysisSnapshot | undefined { return this.snapshotValue ? structuredClone(this.snapshotValue) : undefined; }

  restore(loop: ControlLoopDefinition, recording: LoopRecording | undefined, snapshot: MimoAnalysisSnapshot | undefined): boolean {
    if (!recording || !snapshot || !recording.rawSamples?.length || recording.evidence.recordingId !== snapshot.recordingId || recordingHash(recording) !== snapshot.recordingHash) return false;
    const result = calculateRecordedMimoAnalysis(loop, recording.rawSamples, snapshot.trials, snapshot.options);
    if (result.status === "rejected") return false;
    this.snapshotValue = { ...snapshot, result: structuredClone(result) };
    return true;
  }

  clear(): void { this.snapshotValue = undefined; }
}

export function recordingHash(recording: LoopRecording): string {
  return createHash("sha256").update(JSON.stringify(recording.rawSamples ?? [])).digest("hex");
}
