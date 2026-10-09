import type { ControlLoopDefinition } from "./loopDefinition";
import { RecordingCoordinator, type RecordingRetention, type RecordingSession, type RecordingTransition } from "./recordingCoordinator";
import type { RecordingOwnerPersistence } from "./runtimeRecording";
import type { RecordingRecoveryOwner } from "./runtimeRecording";
import type { LoopRecording } from "./loopMonitor";
import { RECORDING_DURATION_LIMIT_MS } from "./controlStudioLimits";

/**
 * Owns the runtime acquisition paired with a bounded Test experiment.
 *
 * This wrapper deliberately reuses the established recording protocol and
 * recovery persistence contract rather than making Test samples from editor
 * monitor observations.
 */
export class ExperimentRecordingOwner {
  private readonly coordinator: RecordingCoordinator;

  constructor(start: (loop: ControlLoopDefinition, persistence?: RecordingOwnerPersistence, plannedDurationMs?: number) => Promise<RecordingSession>) {
    this.coordinator = new RecordingCoordinator(start);
  }

  get recording(): boolean { return this.coordinator.recording; }
  get loop(): ControlLoopDefinition | undefined { return this.coordinator.loop; }
  get unresolvedStart() { return this.coordinator.unresolvedStart; }
  get recoveryOwner(): RecordingRecoveryOwner | undefined {
    const session = this.coordinator.session as (RecordingSession & { recoveryOwner?: () => RecordingRecoveryOwner }) | undefined;
    return session?.recoveryOwner?.();
  }

  /** Reloaded Test recovery may only stop the original runtime buffer. */
  adoptRecoveredOwner(session: RecordingSession, reason: string): void {
    this.coordinator.adoptRecoveredOwner(session, reason);
  }

  async start(loop: ControlLoopDefinition, persistence?: RecordingOwnerPersistence, plannedDurationMs?: number): Promise<RecordingTransition> {
    if (this.coordinator.recording) throw new Error("A Test recording is already active for this loop.");
    const transition = await this.coordinator.execute("start", loop, persistence, plannedDurationMs);
    if (!transition) throw new Error("The Test recording could not start.");
    return transition;
  }

  /** `retain` keeps the stopped samples in the study first; the runtime buffer is released only after it resolves (E4.7, E11.6). */
  async stop(loop: ControlLoopDefinition, retain?: RecordingRetention): Promise<RecordingTransition> {
    const transition = await this.coordinator.execute("stop", loop, undefined, undefined, retain);
    if (!transition) throw new Error("No matching Test recording is active for this loop.");
    return transition;
  }
}

/** Begins the runtime-owned acquisition before an approved Test writes anything. */
/**
 * The runtime window of a Test recording: its planned duration plus a restoration margin of at least 10 s or 10 % of the
 * duration, whichever is larger, capped at the recording limit (A7).
 */
export function experimentRecordingWindowMs(plannedDurationMs: number): number {
  return Math.min(RECORDING_DURATION_LIMIT_MS, plannedDurationMs + Math.max(10_000, plannedDurationMs * 0.1));
}

/**
 * Starts the recording, then the experiment. The recording identity is journaled by `persistence` before the recording Start is sent
 * (E11.2), and the experiment's record is journaled before the experiment is sent (E11.2, E11.3). Without a journal nothing is dispatched.
 */
export async function startExperimentWithRecording<T>(owner: ExperimentRecordingOwner, loop: ControlLoopDefinition,
  startExperiment: () => Promise<T>, persistence: RecordingOwnerPersistence | undefined,
  persistAcknowledgedOwner?: (owner: RecordingRecoveryOwner) => Promise<void>, plannedDurationMs?: number): Promise<T> {
  if (!persistence) throw new Error("The Test recording needs its journal before Start is sent. No runtime request was made.");
  const started = await owner.start(loop, persistence, plannedDurationMs);
  if (started.kind !== "started") {
    if (owner.recording) {
      try { await owner.stop(loop); }
      catch { /* The original owner remains recoverable; it never grants Test authority. */ }
    }
    throw new Error(started.kind === "start-unconfirmed"
      ? `Test recording Start acknowledgement was not confirmed. ${started.error.message}`
      : "The Test recording could not start.");
  }
  try {
    if (persistAcknowledgedOwner) {
      const recovered = owner.recoveryOwner;
      if (!recovered) throw new Error("The acknowledged Test recording does not expose a recoverable owner.");
      await persistAcknowledgedOwner(recovered);
    }
  } catch (error) {
    try { await owner.stop(loop); }
    catch (stopError) {
      throw new Error(`${error instanceof Error ? error.message : String(error)} Test recording Stop is unconfirmed: ${stopError instanceof Error ? stopError.message : String(stopError)}`);
    }
    throw error;
  }
  try { return await startExperiment(); }
  catch (error) {
    try { await owner.stop(loop); }
    catch (stopError) {
      throw new Error(`${error instanceof Error ? error.message : String(error)} Test recording Stop is unconfirmed: ${stopError instanceof Error ? stopError.message : String(stopError)}`);
    }
    throw error;
  }
}

/** Stops the exact Test acquisition and returns only the runtime-owned record. */
export async function stopExperimentRecording(owner: ExperimentRecordingOwner,
  loop: ControlLoopDefinition, retain?: RecordingRetention): Promise<LoopRecording> {
  const stopped = await owner.stop(loop, retain);
  if (stopped.kind !== "stopped") throw new Error("The Test recording did not produce terminal evidence.");
  return stopped.recording;
}

/** Reconnects only the persisted Test buffer; it intentionally exposes no Start path. */
export async function restoreExperimentRecordingOwner(owner: ExperimentRecordingOwner, loop: ControlLoopDefinition,
  persisted: RecordingRecoveryOwner,
  recover: (loop: ControlLoopDefinition, owner: RecordingRecoveryOwner) => Promise<RecordingSession>): Promise<void> {
  owner.adoptRecoveredOwner(await recover(loop, persisted),
    "Test recording recovery unconfirmed — Stop the original runtime buffer to retain terminal Test evidence.");
}
