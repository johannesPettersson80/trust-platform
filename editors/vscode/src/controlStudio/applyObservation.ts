import { displayEngineeringUnit, formatEngineeringNumber } from "./chartContract";
import { assessCommissioningRecording, type CommissioningCriteria } from "./commissioningAssessment";
import type { CommissioningCheckpoint, CommissioningCheckpointWriter } from "./commissioningCheckpoint";
import { CONNECTION_LOSS_TIMEOUT_MS } from "./controlStudioLimits";
import type { ControlLoopDefinition, ControlSignalBinding } from "./loopDefinition";
import type { LoopRecording } from "./loopMonitor";
import { MainWorkflowError } from "./mainWorkflowFailure";
import { RuntimeControlError } from "../runtimeControlClient";
import { RuntimeRecordingSession, type RecordingFollowPage, type RecordingOwnerPersistence, type RecordingRecoveryOwner } from "./runtimeRecording";
import type { WorkflowResultSample } from "./workflowResult";
import type { TransferStatus } from "./runtimeTransfer";

/**
 * A measured output's permitted range (Specification 35, B6.3): the output's declared range, and the recorded channel that carries
 * its value. The process variable is recorded as `sample.pv`; any other measured output by its auxiliary channel id (B1.4).
 */
export interface PermittedRange {
  readonly symbol: string;
  readonly unit: string;
  readonly minimum: number;
  readonly maximum: number;
  /** The auxiliary channel that records this output; absent for the process variable. */
  readonly auxiliaryId?: string;
}

/**
 * Every measured output of the loop with its own permitted range, the process variable first (B6.3). The measured outputs are the
 * process variable and the auxiliary channels of role PV and state (B1.4); no other auxiliary channel is an output.
 */
export function observedRangesOf(loop: Pick<ControlLoopDefinition, "signals">): readonly PermittedRange[] {
  const auxiliary = (loop.signals.additional ?? []).filter(signal => signal.role === "PV" || signal.role === "state");
  return [rangeOf(loop.signals.processVariable), ...auxiliary.map(signal => ({ ...rangeOf(signal), auxiliaryId: signal.id }))];
}

function rangeOf(signal: ControlSignalBinding): PermittedRange {
  return { symbol: signal.symbol, unit: signal.unit, minimum: signal.range.minimum, maximum: signal.range.maximum };
}

/** The applied response left its permitted range (B6.3): the previous settings are restored at once, and B6.7 offers no action. */
export class PermittedRangeLeft extends MainWorkflowError {
  /** The recording the runtime held up to the departure, once it is stored; a reattached observation carries it (B6.3). */
  constructor(readonly statement: string, readonly recording?: LoopRecording) {
    super(statement, { cause: "model-or-data", nextAction: "none" });
    this.name = "PermittedRangeLeft";
  }
}

/** A recorded response that failed a requirement (B6.3): the applied settings stay in place and Restore previous settings is offered. */
export class ObservationRequirementFailed extends MainWorkflowError {
  /** A reattached observation carries the recording it assessed, so the applied settings can be reported with Restore (B6.3). */
  constructor(message: string, readonly recording?: LoopRecording) {
    super(message, { cause: "settings", nextAction: "restore" });
    this.name = "ObservationRequirementFailed";
  }
}

/** Closing the panel detached a reattached observation: its client work ends, and the runtime keeps recording it (A8.5, A10.4). */
export class ObservationDetached extends Error {
  constructor() {
    super("The panel closed while the observation was reattached; the runtime keeps recording it.");
    this.name = "ObservationDetached";
  }
}

/** The runtime no longer holds the recording, for example after a restart: the observation is incomplete and Restore is offered (A8.6). */
export class ObservationNotHeld extends MainWorkflowError {
  constructor(runtimeReason: string) {
    super(`The runtime no longer holds the recording of this Apply observation, so the observation is incomplete. ${runtimeReason}`,
      { cause: "runtime", nextAction: "restore" });
    this.name = "ObservationNotHeld";
  }
}

/** The runtime's answer that it no longer holds a recording, or that the recording cannot continue (runtime recording_unavailable). */
export function recordingNotHeld(error: unknown): error is RuntimeControlError {
  return error instanceof RuntimeControlError && error.code === "recording_unavailable";
}

/** The stored evidence of a recording the runtime no longer holds: incomplete, with no samples (A8.6). */
export function incompleteRecording(reason: string): LoopRecording {
  return { samples: [], rawSamples: [], evidence: { method: "runtime_scan_buffer", completeness: "incomplete", observedSampleCount: 0,
    missingScanRanges: [], droppedObservedSamples: 0, actualCycleTimingAvailable: false, freshnessAvailable: false, reason } };
}

/** Stops a retained recording for a recovery. One the runtime no longer holds is stored as incomplete evidence, and `held` is false. */
export async function stopRecovered(session: RecordingFollower): Promise<{ recording: LoopRecording; held: boolean }> {
  try { return { recording: await session.stop(), held: true }; }
  catch (error) {
    if (!recordingNotHeld(error)) throw error;
    return { recording: incompleteRecording(`The runtime no longer holds this recording. ${error.message}`), held: false };
  }
}

/** Assesses the recorded response against the requirements; every failure of an assessment is a failed requirement. */
export function assessObservation(recording: LoopRecording, criteria: CommissioningCriteria): number {
  try { return assessCommissioningRecording(recording, criteria); }
  catch (error) { throw new ObservationRequirementFailed(error instanceof Error ? error.message : String(error)); }
}

/** The part of a runtime recording that an observation follows in process time, stops, and releases once its samples are stored. */
export interface RecordingFollower {
  follow(): Promise<RecordingFollowPage>;
  stop(): Promise<LoopRecording>;
  release(): Promise<void>;
}

export interface ObservationFollowOptions {
  readonly durationMs: number;
  /** Every measured output with its own permitted range; each one is checked on every sample (B6.3). */
  readonly ranges: readonly PermittedRange[];
  readonly checkCancelled: () => void;
  readonly progress?: (text: string) => void | Promise<void>;
  /** This receiver's wall clock for the connection-loss rule (A3.4); tests inject it. */
  readonly wallClockMs?: () => number;
  readonly pollMs?: number;
  /** When given, the response starts at the first sample this accepts, and the recording is stopped as soon as the response
   * covers `durationMs` of process time from there. Otherwise the recording runs to its own end. */
  readonly responseStart?: (sample: WorkflowResultSample) => boolean;
}

/**
 * Follows a running recording to its end in process time (B6.3, A3.6). Every sample is checked against the permitted range of each
 * measured output as it arrives, so a departure by any output ends the observation at once. The recording is then stopped and returned whole.
 */
export async function followAppliedResponse(recording: RecordingFollower, options: ObservationFollowOptions): Promise<LoopRecording> {
  const wallClockMs = options.wallClockMs ?? Date.now;
  let newestElapsedMs = -1;
  let newestProgressMs = wallClockMs();
  let responseStartSeconds: number | undefined;
  let newestSampleSeconds = Number.NEGATIVE_INFINITY;
  for (;;) {
    options.checkCancelled();
    const page = await recording.follow();
    if (page.state === "failed") {
      throw new MainWorkflowError(`The runtime ended the recording${page.reason ? `: ${page.reason}` : ""}. Connect the runtime again before any further live action.`,
        { cause: "runtime", nextAction: "connect-runtime" });
    }
    checkPermittedRanges(page.rawSamples, options.ranges);
    for (const sample of page.rawSamples) {
      if (responseStartSeconds === undefined && options.responseStart?.(sample)) responseStartSeconds = sample.elapsedSeconds;
      newestSampleSeconds = Math.max(newestSampleSeconds, sample.elapsedSeconds);
    }
    if (page.elapsedMs > newestElapsedMs) {
      newestElapsedMs = page.elapsedMs;
      newestProgressMs = wallClockMs();
    } else if (wallClockMs() - newestProgressMs > CONNECTION_LOSS_TIMEOUT_MS) {
      throw new MainWorkflowError("The runtime stopped sending values.", { cause: "runtime", nextAction: "connect-runtime" });
    }
    // The response covers the reviewed duration from its own start, so the time the target change took is never cut from it.
    const responseMs = responseStartSeconds === undefined ? 0 : (newestSampleSeconds - responseStartSeconds) * 1_000;
    if (responseStartSeconds !== undefined && responseMs >= options.durationMs) break;
    if (page.state !== "recording" && page.nextSample >= page.sampleCount) break;
    if (page.state === "recording") {
      const elapsedMs = options.responseStart ? responseMs : page.elapsedMs;
      const remainingMs = Math.max(0, options.durationMs - elapsedMs);
      await options.progress?.(`Recording the response: ${seconds(elapsedMs)} s of ${seconds(options.durationMs)} s elapsed, `
        + `${seconds(remainingMs)} s of process time remaining.`);
      await new Promise(resolve => setTimeout(resolve, options.pollMs ?? 250));
    }
  }
  const stopped = await recording.stop();
  checkPermittedRanges(stopped.rawSamples ?? [], options.ranges);
  return stopped;
}

/** The sentence that names a permitted-range departure, as B6.7 words it, before the restoration is stated. */
export function permittedRangeStatement(range: PermittedRange, value: number): string {
  const unit = displayEngineeringUnit(range.unit);
  const suffix = unit ? ` ${unit}` : "";
  return `${range.symbol} left its permitted range (${formatEngineeringNumber(value)}${suffix} outside `
    + `${formatEngineeringNumber(range.minimum)}${suffix}..${formatEngineeringNumber(range.maximum)}${suffix})`;
}

/** Checks every measured output in every sample, in declared order. The first output that leaves its range is named (B6.3, B6.7). */
function checkPermittedRanges(samples: readonly WorkflowResultSample[], ranges: readonly PermittedRange[]): void {
  for (const sample of samples) {
    for (const range of ranges) {
      const value = recordedValueOf(sample, range);
      if (value !== null && (value < range.minimum || value > range.maximum)) throw new PermittedRangeLeft(permittedRangeStatement(range, value));
    }
  }
}

/** The output's value in a sample: `sample.pv` for the process variable, else its auxiliary channel by id; null when not recorded. */
function recordedValueOf(sample: WorkflowResultSample, range: PermittedRange): number | null {
  return range.auxiliaryId === undefined ? sample.pv : (sample.additional?.find(channel => channel.id === range.auxiliaryId)?.value ?? null);
}

function seconds(milliseconds: number): string {
  return formatEngineeringNumber(milliseconds / 1000);
}

/** What reopening needs about a running observation: where it started in process time and how long it is planned to last (A8.5). */
export interface ObservationJournal {
  readonly startedAtProcessNs: number;
  readonly durationMs: number;
  /** The confirmed transfers of the Apply before its target change; a reopened observation's receipt carries them (B6.3, A8.6). */
  readonly transfers?: readonly TransferStatus[];
}

/** A checkpoint written while an observation is journalled also carries that observation's identity. */
export type ObservedCheckpoint = CommissioningCheckpoint & { observation?: ObservationJournal };

/**
 * Journals the observation through the commissioning journal's own writer: every checkpoint written after `begin` carries the
 * observation identity, so the identities are durable before the recording and the target change are dispatched.
 */
export class ObservationRecord {
  private current: ObservationJournal | undefined;
  constructor(private readonly writer: CommissioningCheckpointWriter) {}

  begin(startedAtProcessNs: number, durationMs: number, transfers: readonly TransferStatus[] = []): void {
    this.current = { startedAtProcessNs, durationMs, ...(transfers.length ? { transfers: structuredClone([...transfers]) } : {}) };
  }

  checkpointWriter(): CommissioningCheckpointWriter {
    return async checkpoint => {
      const journalled: ObservedCheckpoint = this.current ? { ...checkpoint, observation: this.current } : checkpoint;
      await this.writer(journalled);
    };
  }
}

/** One observed target change: its journal entry, its recording, and the change that starts the response. */
export interface TargetObservationPlan {
  readonly durationMs: number;
  /** Process time at which the observation starts: the runtime clock of the Auto entry. */
  readonly startedAtNs: number;
  readonly persistence?: RecordingOwnerPersistence;
  readonly journal?: ObservationRecord;
  /** Reports the process time elapsed and remaining while the response is recorded, as a reattached observation does (B6.3, A3.6). */
  readonly progress?: (text: string) => void | Promise<void>;
  readonly checkCancelled: () => void;
  readonly onStarted: (session: RuntimeRecordingSession) => void;
  readonly applyTarget: () => Promise<unknown>;
  /** The first sample of the response: the controller in Auto at the reviewed target, as the assessment defines it. */
  readonly responseStart: (sample: WorkflowResultSample) => boolean;
  /** The Apply's confirmed transfers before the target change, journalled with the observation. */
  readonly transfers?: readonly TransferStatus[];
  /** The requirements' settling time: the observation is recorded at a record period that follows the process (A4.2). */
  readonly expectedSettlingSeconds?: number;
}

/**
 * Specification 35, B6.3: the recording starts before the validated target change, which then runs, and the response is
 * recorded for the test duration in process time. The recording is owned by the runtime and followed to its end.
 */
export async function observeTargetChange(loop: ControlLoopDefinition, plan: TargetObservationPlan): Promise<LoopRecording> {
  plan.journal?.begin(plan.startedAtNs, plan.durationMs, plan.transfers);
  // The recording starts before the target change, so it also holds the change's confirmation window (A7) and two task periods;
  // it is stopped as soon as the response covers the reviewed duration, and the runtime ends it there at the latest.
  const task = loop.controller.taskPeriodMs;
  const plannedMs = plan.durationMs + Math.max(10_000, 3 * task + CONNECTION_LOSS_TIMEOUT_MS) + 2 * task;
  const session = await RuntimeRecordingSession.start(loop, plan.persistence, plannedMs, plan.expectedSettlingSeconds);
  plan.onStarted(session);
  plan.checkCancelled();
  await plan.applyTarget();
  return followAppliedResponse(session, { durationMs: plan.durationMs, ranges: observedRangesOf(loop), checkCancelled: plan.checkCancelled,
    progress: plan.progress, responseStart: plan.responseStart });
}

export interface ResumedObservation {
  readonly recording: LoopRecording;
  readonly maximumError: number;
}

export interface ResumeOptions {
  /** Reopens the runtime's recording by its identity; by default the runtime is asked for it. */
  readonly open?: (owner: RecordingRecoveryOwner) => Promise<RecordingFollower>;
  /** Stores the recovered samples, for example in the study. The runtime's buffer is released only after this returns. */
  readonly retain: (recording: LoopRecording) => Promise<void>;
  /** Reports the process time elapsed and remaining while the recording runs, as the Validate page shows it (B6.3). */
  readonly progress?: (text: string) => void | Promise<void>;
  /** Whether the panel has closed: a detached observation stops following and touches nothing in the runtime (A10.4). */
  readonly detached?: () => boolean;
}

/** Whether a checkpoint still journals an observation whose recording the runtime may hold: the identity is kept until release (A8.5). */
export function observationInProgress(checkpoint: CommissioningCheckpoint): boolean {
  const observed = checkpoint as ObservedCheckpoint;
  return Boolean(observed.observation && observed.recording) && !["completed", "recovered", "retired"].includes(checkpoint.stage);
}

/**
 * Resumes a journalled observation after a reopening (A8.5, A8.6, A10.4): follows the runtime's recording to its end, stores its
 * samples, and only then releases the runtime's buffer (runtimeRecording.ts). It replays no request and writes no controller value;
 * the caller decides what the assessed result means. A departure from the permitted range stores the samples too, and the caller
 * then restores the previous settings at once (B6.3).
 */
export async function resumeApplyObservation(loop: ControlLoopDefinition, checkpoint: ObservedCheckpoint, criteria: CommissioningCriteria,
  options: ResumeOptions): Promise<ResumedObservation> {
  if (!checkpoint.observation || !checkpoint.recording) throw new Error("The commissioning journal holds no applied response to resume.");
  try {
    const session = await openResumedRecording(loop, checkpoint.recording, options.open);
    const recording = await followResumed(loop, session, checkpoint.observation.durationMs, options);
    await options.retain(recording);
    await session.release();
    return assessResumed(recording, criteria);
  } catch (error) {
    if (recordingNotHeld(error)) throw new ObservationNotHeld(error.message);
    throw error;
  }
}

/** Reopens the runtime's recording by its identity. A runtime that cannot be reached is a connection failure, not an incomplete observation. */
async function openResumedRecording(loop: ControlLoopDefinition, owner: RecordingRecoveryOwner,
  open: ResumeOptions["open"]): Promise<RecordingFollower> {
  try { return await (open ?? ((recovering: RecordingRecoveryOwner) => RuntimeRecordingSession.recover(loop, recovering)))(owner); }
  catch (error) { throw new MainWorkflowError(error instanceof Error ? error.message : String(error), { cause: "runtime", nextAction: "connect-runtime" }); }
}

/** Follows the recording to its end in process time. A departure from the permitted range is stored before it is raised (B6.3). */
async function followResumed(loop: ControlLoopDefinition, session: RecordingFollower, durationMs: number, options: ResumeOptions): Promise<LoopRecording> {
  const detached = options.detached ?? (() => false);
  try {
    return await followAppliedResponse(session, { durationMs, ranges: observedRangesOf(loop), progress: options.progress,
      checkCancelled: () => { if (detached()) throw new ObservationDetached(); } });
  } catch (error) {
    if (error instanceof PermittedRangeLeft) throw await storeDeparture(error, session, options.retain);
    throw error;
  }
}

/** Stores the samples the runtime holds at a departure, before the caller restores the settings. A recording the runtime no longer holds has none. */
async function storeDeparture(departure: PermittedRangeLeft, session: RecordingFollower,
  retain: (recording: LoopRecording) => Promise<void>): Promise<PermittedRangeLeft> {
  let stopped: LoopRecording | undefined;
  try { stopped = await session.stop(); }
  catch (error) { if (!recordingNotHeld(error)) throw error; }
  if (!stopped) return departure;
  await retain(stopped);
  await session.release();
  return new PermittedRangeLeft(departure.statement, stopped);
}

/** Assesses the stored response. A failed requirement keeps its recording, so the caller can report the settings as applied (B6.3). */
function assessResumed(recording: LoopRecording, criteria: CommissioningCriteria): ResumedObservation {
  try { return { recording, maximumError: assessObservation(recording, criteria) }; }
  catch (error) { throw error instanceof ObservationRequirementFailed ? new ObservationRequirementFailed(error.message, recording) : error; }
}
