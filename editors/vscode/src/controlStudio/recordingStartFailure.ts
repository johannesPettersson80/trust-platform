import type { RecordingSession } from "./recordingCoordinator";
import type { LoopRecording } from "./loopMonitor";

/** An uncertain acknowledgement never erases the already-created recording owner. */
export class RecordingStartUnconfirmed extends Error {
  constructor(readonly session: RecordingSession, readonly recording: LoopRecording | undefined,
    startError: unknown, stopError?: unknown) {
    super(`Recording Start acknowledgement was not confirmed: ${String(startError)}. ${recording
      ? "The existing recording was stopped and its actual samples retained."
      : `Stop remains unconfirmed: ${String(stopError)}. Retry Stop for this recording.`}`);
    this.name = "RecordingStartUnconfirmed";
  }
}
