import type { ControlLoopDefinition } from "./loopDefinition";
import type { LoopRecording, LoopRecordingEvidence } from "./loopMonitor";
import { RecordingStartUnconfirmed } from "./recordingStartFailure";
import type { RecordingOwnerPersistence } from "./runtimeRecording";

export interface RecordingSession {
  readonly loop: ControlLoopDefinition;
  stop(): Promise<LoopRecording>;
  /** Frees the stopped runtime buffer. Called only after the stopped samples are kept (E4.7, E11.6). */
  release?(): Promise<void>;
}
/** Keeps a stopped recording's samples, for example in the study. The runtime buffer is released only after this resolves. */
export type RecordingRetention = (recording: LoopRecording) => Promise<void>;
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
  /** `plannedDurationMs` sizes the runtime window; without it the start keeps its one-hour default. */
  constructor(private readonly start: (loop: ControlLoopDefinition, persistence?: RecordingOwnerPersistence, plannedDurationMs?: number) => Promise<RecordingSession>) {}
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

  async execute(action: "start" | "stop", loop: ControlLoopDefinition, persistence?: RecordingOwnerPersistence, plannedDurationMs?: number,
    retain?: RecordingRetention): Promise<RecordingTransition | undefined> {
    if (action === "start") {
      if (this.recording) return undefined;
      this.unconfirmedStart = undefined;
      this.pendingLoop = loop;
      try {
        this.pendingStart = this.start(loop, persistence, plannedDurationMs);
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
    const pending = this.unconfirmedStart;
    if (pending?.recording) {
      await keepAndRelease(pending.session, pending.recording, retain);
      this.unconfirmedStart = undefined;
      return { kind: "stopped", recording: pending.recording };
    }
    if (!this.session && this.recoveredReason) throw new Error(this.recoveredReason);
    if (this.stopping || !this.session) return undefined;
    const session = this.session;
    this.stopping = true;
    try {
      const recording = await session.stop();
      // A failed keep leaves this owner in place, so the next Stop retries the keep before any release (E4.7, E11.6).
      await keepAndRelease(session, recording, retain);
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

/** Keeps the stopped samples, then frees the runtime buffer. Without a keeper the buffer stays for the next start's sweep. */
async function keepAndRelease(session: RecordingSession, recording: LoopRecording, retain: RecordingRetention | undefined): Promise<void> {
  if (!retain) return;
  await retain(recording);
  await session.release?.();
}

function recordingStarted(): RecordingStateNotice {
  return { state: "recording", reason: "Recording at runtime — Start acknowledged; waiting for Stop to retain runtime scans." };
}

function recordingUnconfirmed(error: RecordingStartUnconfirmed): RecordingStateNotice {
  return { state: "unconfirmed", reason: error.message };
}

export function sameRecordingScope(left: ControlLoopDefinition | undefined, right: ControlLoopDefinition | undefined): boolean {
  return !!left && !!right && left.projectId === right.projectId && left.runtimeId === right.runtimeId
    && left.id === right.id && left.revision === right.revision;
}
