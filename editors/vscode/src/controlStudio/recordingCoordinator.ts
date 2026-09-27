import type { ControlLoopDefinition } from "./loopDefinition";
import type { LoopRecording, LoopRecordingEvidence } from "./loopMonitor";
import { RecordingStartUnconfirmed } from "./recordingStartFailure";
import type { RecordingOwnerPersistence } from "./runtimeRecording";

export interface RecordingSession {
  readonly loop: ControlLoopDefinition;
  stop(): Promise<LoopRecording>;
}
export interface RecordingStateNotice {
  readonly state: "starting" | "recording" | "unconfirmed";
  readonly reason: string;
}
export type RecordingUpdate = LoopRecordingEvidence | RecordingStateNotice;
export type RecordingTransition = { kind: "started" } | { kind: "stopped"; recording: LoopRecording }
  | { kind: "start-unconfirmed"; error: RecordingStartUnconfirmed };

/** Owns the active recording transaction independently of panel rendering. */
export class RecordingCoordinator {
  session?: RecordingSession;
  private pendingStart?: Promise<RecordingSession>;
  private pendingLoop?: ControlLoopDefinition;
  private unconfirmedStart?: RecordingStartUnconfirmed;
  private recoveredReason?: string;
  private recoveredLoop?: ControlLoopDefinition;
  private stopping = false;
  constructor(private readonly start: (loop: ControlLoopDefinition, persistence?: RecordingOwnerPersistence) => Promise<RecordingSession>) {}
  get recording(): boolean { return !!this.pendingLoop || !!this.session || !!this.recoveredLoop || this.stopping; }
  get loop(): ControlLoopDefinition | undefined { return this.session?.loop ?? this.pendingLoop ?? this.recoveredLoop; }
  get unresolvedStart(): RecordingStartUnconfirmed | undefined { return this.unconfirmedStart; }
  get currentUpdate(): RecordingStateNotice | undefined {
    if (this.unconfirmedStart) return recordingUnconfirmed(this.unconfirmedStart);
    if (this.recoveredReason) return { state: "unconfirmed", reason: this.recoveredReason };
    if (this.pendingLoop) return { state: "starting", reason: "Starting recording…" };
    return this.session ? recordingStarted() : undefined;
  }

  /** A reconstructed host-lifetime owner has no Start authority; it may only Stop the retained ID. */
  adoptRecoveredOwner(session: RecordingSession, reason: string): void {
    const retryingUnavailableOwner = !this.session && !!this.recoveredLoop &&
      sameRecordingScope(this.recoveredLoop, session.loop);
    if ((!retryingUnavailableOwner && this.recording) || !reason.trim()) {
      throw new Error("A recording owner is already active or recovery reason is invalid.");
    }
    this.session = session;
    this.recoveredLoop = session.loop;
    this.recoveredReason = reason;
  }

  adoptUnavailableRecovery(loop: ControlLoopDefinition, reason: string): void {
    const retryingUnavailableOwner = !this.session && !!this.recoveredLoop &&
      sameRecordingScope(this.recoveredLoop, loop);
    if ((!retryingUnavailableOwner && this.recording) || !reason.trim()) {
      throw new Error("A recording owner is already active or recovery reason is invalid.");
    }
    this.recoveredLoop = loop;
    this.recoveredReason = reason;
  }

  /** Keeps the original recovered owner blocked while exposing its rejected terminal evidence. */
  reportRecoveredStopFailure(loop: ControlLoopDefinition, error: unknown): boolean {
    if (!this.session || !this.recoveredLoop || !sameRecordingScope(this.recoveredLoop, loop)) return false;
    this.recoveredReason = `Recording recovery unconfirmed — ${error instanceof Error ? error.message : String(error)}`;
    return true;
  }

  async execute(action: "start" | "stop", loop: ControlLoopDefinition, persistence?: RecordingOwnerPersistence): Promise<RecordingTransition | undefined> {
    if (action === "start") {
      if (this.recording) return undefined;
      this.unconfirmedStart = undefined;
      this.pendingLoop = loop;
      try {
        this.pendingStart = this.start(loop, persistence);
        this.session = await this.pendingStart;
        return { kind: "started" };
      } catch (error) {
        if (error instanceof RecordingStartUnconfirmed) {
          this.retainUnconfirmedStart(error);
          return { kind: "start-unconfirmed", error };
        }
        throw error;
      } finally { this.pendingStart = undefined; this.pendingLoop = undefined; }
    }
    const bound = this.loop;
    if (!sameRecordingScope(bound, loop)) {
      throw new Error("No matching runtime recording is active for this scope.");
    }
    if (this.pendingStart) {
      try { await this.pendingStart; }
      catch (error) {
        if (error instanceof RecordingStartUnconfirmed) this.retainUnconfirmedStart(error);
        else throw error;
      }
    }
    if (this.unconfirmedStart?.recording) {
      const recording = this.unconfirmedStart.recording;
      this.unconfirmedStart = undefined;
      return { kind: "stopped", recording };
    }
    if (!this.session && this.recoveredReason) throw new Error(this.recoveredReason);
    if (this.stopping || !this.session) return undefined;
    const session = this.session;
    this.stopping = true;
    try {
      const recording = await session.stop();
      if (this.session === session) {
        this.session = undefined;
        this.unconfirmedStart = undefined;
        this.recoveredReason = undefined;
        this.recoveredLoop = undefined;
      }
      return { kind: "stopped", recording };
    } finally { this.stopping = false; }
  }

  private retainUnconfirmedStart(error: RecordingStartUnconfirmed): void {
    this.unconfirmedStart = error;
    this.session = error.recording ? undefined : error.session;
  }
}

export function recordingFailure(error: unknown): LoopRecordingEvidence {
  return { method: "runtime_scan_buffer", completeness: "incomplete", observedSampleCount: 0,
    missingScanRanges: [], droppedObservedSamples: 0, actualCycleTimingAvailable: false, freshnessAvailable: false,
    reason: `Incomplete recording — ${error instanceof Error ? error.message : String(error)}` };
}

function recordingStarted(): RecordingStateNotice {
  return { state: "recording", reason: "Recording at runtime — Start acknowledged; waiting for Stop to retain runtime scans." };
}

function recordingUnconfirmed(error: RecordingStartUnconfirmed): RecordingStateNotice {
  return { state: "unconfirmed", reason: error.message };
}

export function isRecordingStateNotice(value: unknown): value is RecordingStateNotice {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const state = (value as RecordingStateNotice).state;
  return state === "starting" || state === "recording" || state === "unconfirmed";
}

/** Shared recording completion owns state notifications and terminal evidence. */
export async function executeRecordingCommand(coordinator: RecordingCoordinator, action: "start" | "stop",
  loop: ControlLoopDefinition, hooks: {
    recordingChanged(value: boolean): void; started(): void; stopped(recording: LoopRecording): void;
    publish(update: RecordingUpdate): Promise<void>; refresh(): void;
    persistOwner?(owner: Parameters<RecordingOwnerPersistence["persist"]>[0]): Promise<void>;
    clearOwner?(): Promise<void>;
  }): Promise<void> {
  const persistence = hooks.persistOwner && hooks.clearOwner
    ? { persist: hooks.persistOwner, clear: async () => hooks.clearOwner!() } satisfies RecordingOwnerPersistence : undefined;
  const transition = coordinator.execute(action, loop, persistence);
  hooks.recordingChanged(coordinator.recording);
  if (action === "start" && coordinator.currentUpdate) await hooks.publish(coordinator.currentUpdate);
  try {
    const result = await transition;
    if (result?.kind === "started") { hooks.started(); await hooks.publish(recordingStarted()); }
    if (result?.kind === "stopped") { hooks.stopped(result.recording); await hooks.clearOwner?.(); await hooks.publish(result.recording.evidence); }
    if (result?.kind === "start-unconfirmed") {
      if (result.error.recording) {
        hooks.stopped(result.error.recording);
        await hooks.clearOwner?.();
        await hooks.publish(result.error.recording.evidence);
      } else await hooks.publish(recordingUnconfirmed(result.error));
    }
  } catch (error) {
    coordinator.reportRecoveredStopFailure(loop, error);
    await hooks.publish(coordinator.currentUpdate ?? (coordinator.unresolvedStart ? recordingUnconfirmed(coordinator.unresolvedStart) : recordingFailure(error)));
  }
  finally { hooks.recordingChanged(coordinator.recording); hooks.refresh(); }
}


export function sameRecordingScope(left: ControlLoopDefinition | undefined, right: ControlLoopDefinition | undefined): boolean {
  return !!left && !!right && left.projectId === right.projectId && left.runtimeId === right.runtimeId
    && left.id === right.id && left.revision === right.revision;
}
