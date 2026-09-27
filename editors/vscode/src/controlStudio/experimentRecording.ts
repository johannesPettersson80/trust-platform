import type { ControlLoopDefinition } from "./loopDefinition";
import { RecordingCoordinator, type RecordingSession, type RecordingTransition } from "./recordingCoordinator";
import type { RecordingOwnerPersistence } from "./runtimeRecording";
import type { RecordingRecoveryOwner } from "./runtimeRecording";
import type { LoopRecording } from "./loopMonitor";

/**
 * Owns the runtime acquisition paired with a bounded Test experiment.
 *
 * Manual Record has a separate coordinator. This wrapper deliberately reuses
 * the established recording protocol and recovery persistence contract rather
 * than making Test samples from editor monitor observations.
 */
export class ExperimentRecordingOwner {
  private readonly coordinator: RecordingCoordinator;

  constructor(start: (loop: ControlLoopDefinition, persistence?: RecordingOwnerPersistence) => Promise<RecordingSession>) {
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

  async start(loop: ControlLoopDefinition, persistence?: RecordingOwnerPersistence): Promise<RecordingTransition> {
    if (this.coordinator.recording) throw new Error("A Test recording is already active for this loop.");
    const transition = await this.coordinator.execute("start", loop, persistence);
    if (!transition) throw new Error("The Test recording could not start.");
    return transition;
  }

  async stop(loop: ControlLoopDefinition): Promise<RecordingTransition> {
    const transition = await this.coordinator.execute("stop", loop);
    if (!transition) throw new Error("No matching Test recording is active for this loop.");
    return transition;
  }
}

/** Begins the runtime-owned acquisition before an approved Test writes anything. */
export async function startExperimentWithRecording<T>(owner: ExperimentRecordingOwner, loop: ControlLoopDefinition,
  startExperiment: () => Promise<T>, persistence?: RecordingOwnerPersistence,
  persistAcknowledgedOwner?: (owner: RecordingRecoveryOwner) => Promise<void>): Promise<T> {
  const started = await owner.start(loop, persistence);
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
  loop: ControlLoopDefinition): Promise<LoopRecording> {
  const stopped = await owner.stop(loop);
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
