import { RecordingStartUnconfirmed } from "./recordingStartFailure";
import { inspectSymbolicSignals } from "./symbolicInspection";
import { controllerModeAddress } from "./controllerModeBinding";
import { loopInspectionBindings } from "./loopInspectionBindings";
import { captureAdditionalChannels } from "./recordingChannels";
import { randomUUID } from "crypto";
import { runtimeLifecycleService } from "../runtimeLifecycle";
import { sendRuntimeControlRequest, RuntimeControlError } from "../runtimeControlClient";
import { getSelectedRuntimeId } from "../selectedRuntime";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { LoopRecording } from "./loopMonitor";
import type { WorkflowRecordingAcquisition, WorkflowResultSample } from "./workflowResult";
import { simulationTarget } from "./simulationTarget";
import { RECORDING_DURATION_LIMIT_MS, RECORDING_SAMPLE_LIMIT, recordPeriodExecutions } from "./controlStudioLimits";
import { steadyRecordPeriod } from "./recordSequence";

export { recordPeriodExecutions };

type Binding = { id: string; address?: string; symbol?: string; iec_type: string };
type CapturedSample = { scan: number; runtime_time_ns: number; captured_at_ms: number; values: (number | null)[]; forced: boolean[]; quality?: ("good" | "bad" | "forced" | "missing")[] };

/** A recording without a planned duration covers the one-hour window it has always covered. */
const DEFAULT_RECORDING_DURATION_MS = 3_600_000;
/** A follow page holds at most this many samples (runtime protocol annex). */
const FOLLOW_PAGE_SAMPLES = 2_000;


/** One page of a running recording, decoded as the stop result decodes its samples. */
export interface RecordingFollowPage {
  state: string;
  elapsedMs: number;
  sampleCount: number;
  complete: boolean;
  reason: string | null;
  rawSamples: WorkflowResultSample[];
  nextSample: number;
}

/** Persistable identity for a future extension-host recovery; credentials and recording data stay out. */
export interface RecordingRecoveryOwner {
  schemaVersion: 1;
  recordingId: string;
  projectId: string;
  runtimeId: string;
  loopId: string;
  loopRevision: string;
  source: "Simulation" | "Physical";
  programRevision?: string;
  /** Start reached the runtime but its acknowledgement was lost before this host ended. */
  startAcknowledgementUnconfirmed?: true;
  bindings: readonly { id: string; address?: string; symbol?: string; iec_type: string }[];
}

export interface RecordingOwnerPersistence {
  persist(owner: RecordingRecoveryOwner): Promise<void>;
  clear(owner: RecordingRecoveryOwner): Promise<void>;
}

/** A recording keeps its original endpoint and loop identity for stop/retry/release. */
export class RuntimeRecordingSession {
  private stopped?: LoopRecording;
  private startFailure?: string;
  private released = false;
  /** Where the in-order follow continues: the next sample, and the last decoded sample's identity for the cycle and scan checks. */
  private followCursor?: { next: number; scan: number; runtimeTimeNs: number; firstTimeNs: number };
  private static readonly pendingReleases = new Map<string, { endpoint: string; send: (operation: string, params: unknown) => Promise<unknown>; revision: string }>();
  private constructor(
    readonly loop: ControlLoopDefinition,
    private readonly id: string,
    private readonly bindings: readonly Binding[],
    private readonly source: "Simulation" | "Physical",
    private readonly send: (operation: string, params: unknown) => Promise<unknown>,
    private readonly endpoint = "",
    private readonly programRevision?: string
  ) {}

  recoveryOwner(): RecordingRecoveryOwner {
    return {
      schemaVersion: 1, recordingId: this.id, projectId: this.loop.projectId, runtimeId: this.loop.runtimeId,
      loopId: this.loop.id, loopRevision: this.loop.revision, source: this.source,
      ...(this.programRevision ? { programRevision: this.programRevision } : {}),
      ...(this.startFailure ? { startAcknowledgementUnconfirmed: true as const } : {}),
      bindings: structuredClone(this.bindings),
    };
  }

  /** Recreates only a persisted owner; callers still obtain a current endpoint/token themselves. */
  static fromRecoveryOwner(loop: ControlLoopDefinition, owner: RecordingRecoveryOwner,
    send: (operation: string, params: unknown) => Promise<unknown>, endpoint: string): RuntimeRecordingSession {
    if (owner.projectId !== loop.projectId || owner.runtimeId !== loop.runtimeId || owner.loopId !== loop.id ||
        owner.loopRevision !== loop.revision || !owner.recordingId || !owner.bindings.length || owner.bindings.length > 32) {
      throw new Error("Recording recovery owner does not match the active loop scope.");
    }
    const session = new RuntimeRecordingSession(loop, owner.recordingId, structuredClone(owner.bindings), owner.source, send,
      endpoint, owner.programRevision);
    if (owner.startAcknowledgementUnconfirmed) {
      session.startFailure = "Start acknowledgement was not confirmed before Extension Host reload.";
    }
    return session;
  }

  static async recover(loop: ControlLoopDefinition, owner: RecordingRecoveryOwner): Promise<RuntimeRecordingSession> {
    if (getSelectedRuntimeId() !== loop.runtimeId) throw new Error("The selected runtime does not match this recording recovery owner.");
    const lifecycle = await runtimeLifecycleService.snapshot();
    if (!lifecycle.status.running) throw new Error("Start the matching runtime before retrying recording recovery.");
    const endpoint = lifecycle.status.endpoint;
    const token = await runtimeLifecycleService.controlAuthToken(endpoint);
    return RuntimeRecordingSession.fromRecoveryOwner(loop, owner,
      (operation, params) => sendRuntimeControlRequest(endpoint, token, operation, params, { timeoutMs: 5000 }), endpoint);
  }

  /** The planned duration fixes the record period (A4.2) and the window the runtime records (A7 recording duration). */
  static async start(loop: ControlLoopDefinition, persistence?: RecordingOwnerPersistence,
    plannedDurationMs = DEFAULT_RECORDING_DURATION_MS, expectedSettlingSeconds?: number): Promise<RuntimeRecordingSession> {
    if (!Number.isFinite(plannedDurationMs) || plannedDurationMs <= 0 || plannedDurationMs > RECORDING_DURATION_LIMIT_MS) {
      throw new Error(`A recording lasts at most ${RECORDING_DURATION_LIMIT_MS / 3_600_000} h of process time (A7).`);
    }
    if (getSelectedRuntimeId() !== loop.runtimeId) throw new Error("The selected runtime does not match this loop.");
    const lifecycle = await runtimeLifecycleService.snapshot();
    if (!lifecycle.status.running) throw new Error("Start the runtime before recording.");
    const endpoint = lifecycle.status.endpoint;
    const token = await runtimeLifecycleService.controlAuthToken(endpoint);
    for (const [id, pending] of RuntimeRecordingSession.pendingReleases) {
      if (pending.endpoint !== endpoint) continue;
      try {
        await pending.send("control.recording.release", { recording_id: id, loop_revision: pending.revision });
        RuntimeRecordingSession.pendingReleases.delete(id);
      } catch { /* The new start will report the runtime's retained-buffer limit if cleanup is still unavailable. */ }
    }
    const bindings = loopInspectionBindings(loop);
    const send = (operation: string, params: unknown) => sendRuntimeControlRequest(endpoint, token, operation, params, { timeoutMs: 5000 });
    let programRevision: string | undefined;
    const id = randomUUID();
    for (let startAttempt = 0; startAttempt < 5; startAttempt++) {
      let inspection = bindings.some(binding => binding.symbol) ? await inspectSymbolicSignals(
        (operation, params) => send(operation, params), { loopRevision: loop.revision, expectedProgramRevision: programRevision }, bindings) : undefined;
      if (inspection) programRevision ??= inspection.kind === "pending" ? inspection.programRevision : inspection.sample.programRevision;
      for (let attempt = 0; inspection?.kind === "pending" && attempt < 20; attempt++) {
        await new Promise(resolve => setTimeout(resolve, Math.max(50, Math.min(100, loop.controller.taskPeriodMs))));
        inspection = await inspectSymbolicSignals((operation, params) => send(operation, params),
          { loopRevision: loop.revision, expectedProgramRevision: programRevision }, bindings);
      }
      if (inspection?.kind === "pending") throw new Error("Wait for a current coherent symbolic scan, then retry recording.");
      const session = new RuntimeRecordingSession(loop, id, bindings,
        simulationTarget(lifecycle.status) ? "Simulation" : "Physical", send, endpoint, programRevision);
      await persistence?.persist(session.recoveryOwner());
      try {
        // A loop's values change when its task runs, so the recording samples that task's executions (A4.1). A background program
        // runs in every resource cycle, so its record period counts cycles. The record period follows the process (A4.2).
        const task = loop.parameterBinding?.task_name;
        const scheduled = Boolean(task && !task.startsWith("$"));
        const samplePeriodMs = scheduled ? loop.controller.taskPeriodMs : (lifecycle.status.cycleIntervalMs ?? loop.controller.taskPeriodMs);
        await send("control.recording.start", { recording_id: session.id, loop_revision: loop.revision, bindings,
          ...(programRevision ? { program_revision: programRevision } : {}), maximum_samples: RECORDING_SAMPLE_LIMIT,
          maximum_duration_ms: plannedDurationMs,
          record_every: recordPeriodExecutions(plannedDurationMs, samplePeriodMs, RECORDING_SAMPLE_LIMIT, expectedSettlingSeconds),
          ...(scheduled ? { task } : {}) });
        return session;
      } catch (error) {
        // This explicit rejection occurs before buffer allocation. Never replay an uncertain Start.
        if (programRevision && startAttempt < 4 && error instanceof RuntimeControlError
          && error.code === "recording_unavailable"
          && error.message === "recording_unavailable: a current coherent inspection of these symbolic bindings is required") continue;
        if (error instanceof RuntimeControlError) {
          await persistence?.clear(session.recoveryOwner());
          throw error;
        }
        session.startFailure = error instanceof Error ? error.message : String(error);
        await persistence?.persist(session.recoveryOwner());
        let recording: LoopRecording | undefined; let stopError: unknown;
        try { recording = await session.stop(); } catch (failure) { stopError = failure; }
        throw new RecordingStartUnconfirmed(session, recording, error, stopError);
      }
    }
    throw new Error("Recording could not obtain a coherent symbolic selection.");
  }

  /**
   * Stops the recording and decodes its samples. The runtime keeps the buffer until the caller has stored these samples and
   * calls release() (E4.7, E11.6). A stopped buffer that is never released is swept by the next recording start.
   */
  async stop(): Promise<LoopRecording> {
    if (!this.stopped) {
      const raw = await this.send("control.recording.stop", { recording_id: this.id, loop_revision: this.loop.revision });
      this.stopped = this.decode(raw);
      if (this.startFailure) this.stopped = { ...this.stopped, evidence: { ...this.stopped.evidence,
        completeness: "incomplete",
        reason: `Start acknowledgement unconfirmed (${this.startFailure}); stopped data retained. ${this.stopped.evidence.reason}` } };
    }
    if (!this.released) {
      RuntimeRecordingSession.pendingReleases.set(this.id, { endpoint: this.endpoint, send: this.send, revision: this.loop.revision });
    }
    return this.stopped;
  }

  /** Frees the stopped buffer. Call it only after the samples returned by stop() are stored (E4.7, E11.6). */
  async release(): Promise<void> {
    if (!this.stopped) throw new Error("Stop the recording before releasing it.");
    if (this.released) return;
    try {
      await this.send("control.recording.release", { recording_id: this.id, loop_revision: this.loop.revision });
      this.released = true;
      RuntimeRecordingSession.pendingReleases.delete(this.id);
    } catch {
      // The buffer stays pending: the next start, or a later release, retries the cleanup.
    }
  }

  /** Follows the running recording in order, one page per call (control.recording.status). Each page holds at most
   * FOLLOW_PAGE_SAMPLES samples, decoded as the stop result decodes them; the next call continues at nextSample. */
  async follow(): Promise<RecordingFollowPage> {
    const fromSample = this.followCursor?.next ?? 0;
    const raw = await this.send("control.recording.status", { recording_id: this.id, loop_revision: this.loop.revision, from_sample: fromSample });
    return this.decodeFollow(raw, fromSample);
  }

  private decodeFollow(raw: unknown, fromSample: number): RecordingFollowPage {
    // A status page carries no loop revision (protocol annex E4.6): the request names it, and the runtime refuses a mismatch.
    if (!record(raw) || raw.recording_id !== this.id
        || typeof raw.state !== "string" || raw.state.length === 0
        || typeof raw.elapsed_ms !== "number" || !Number.isFinite(raw.elapsed_ms) || raw.elapsed_ms < 0
        || !Number.isSafeInteger(raw.sample_count) || raw.sample_count < fromSample
        || typeof raw.complete !== "boolean" || !(raw.reason === null || typeof raw.reason === "string")
        || !Array.isArray(raw.samples) || raw.samples.length > FOLLOW_PAGE_SAMPLES
        || raw.next_sample !== fromSample + raw.samples.length || raw.next_sample > raw.sample_count) {
      throw new Error("The runtime recording status is invalid.");
    }
    const cursor = this.followCursor;
    const samples = this.decodeSamples(raw.samples, cursor);
    const firstTimeNs = cursor?.firstTimeNs ?? samples[0]?.runtime_time_ns ?? 0;
    const rawSamples = this.toWorkflowSamples(samples, firstTimeNs, cursor?.runtimeTimeNs);
    const last = samples[samples.length - 1];
    if (last) this.followCursor = { next: raw.next_sample, scan: last.scan, runtimeTimeNs: last.runtime_time_ns, firstTimeNs };
    return { state: raw.state, elapsedMs: raw.elapsed_ms, sampleCount: raw.sample_count, complete: raw.complete,
      reason: raw.reason, rawSamples, nextSample: raw.next_sample };
  }

  private decode(raw: unknown): LoopRecording {
    if (!record(raw) || raw.recording_id !== this.id || raw.loop_revision !== this.loop.revision
        || !Array.isArray(raw.bindings) || raw.bindings.length !== this.bindings.length
        || !raw.bindings.every((binding: unknown, index: number) => record(binding) && binding.id === this.bindings[index].id
          && binding.address === this.bindings[index].address && binding.symbol === this.bindings[index].symbol && binding.iec_type === this.bindings[index].iec_type)
        || !Array.isArray(raw.samples) || raw.samples.length > RECORDING_SAMPLE_LIMIT || !record(raw.acquisition)
        || raw.acquisition.method !== "runtime_scan_buffer" || raw.acquisition.observedSampleCount !== raw.samples.length) {
      throw new Error("The runtime recording identity or acquisition data is invalid.");
    }
    const symbolic = this.bindings.some(binding => binding.symbol);
    if ((symbolic && (typeof raw.program_revision !== "string" || !raw.program_revision))
      || (this.programRevision !== undefined && raw.program_revision !== this.programRevision)) {
      throw new Error("The recording program identity does not match its bound program.");
    }
    const acquisitionValue = raw.acquisition;
    if (!["complete", "incomplete"].includes(String(acquisitionValue.completeness))
        || !Array.isArray(acquisitionValue.missingScanRanges)
        || !acquisitionValue.missingScanRanges.every((range: unknown) => record(range) && Number.isSafeInteger(range.first)
          && Number.isSafeInteger(range.last) && range.first >= 0 && range.last >= range.first)
        || !Number.isSafeInteger(acquisitionValue.droppedObservedSamples) || acquisitionValue.droppedObservedSamples < 0
        || typeof acquisitionValue.actualCycleTimingAvailable !== "boolean" || typeof acquisitionValue.freshnessAvailable !== "boolean"
        || typeof acquisitionValue.reason !== "string"
        || acquisitionValue.completeness === "complete" && (acquisitionValue.missingScanRanges.length > 0
          || acquisitionValue.droppedObservedSamples !== 0 || !acquisitionValue.actualCycleTimingAvailable)) {
      throw new Error("The runtime acquisition claims are inconsistent.");
    }
    const samples = this.decodeSamples(raw.samples);
    const rawSamples = this.toWorkflowSamples(samples, samples[0]?.runtime_time_ns ?? 0);
    const acquisition = raw.acquisition as unknown as WorkflowRecordingAcquisition;
    // Each sample covers one record period of executions (A4.2, E4.4); a stopped recording's final sample may cover fewer.
    const steady = steadyRecordPeriod(samples.map(sample => sample.scan), true);
    const complete = acquisition.completeness === "complete" && samples.length >= 2 && steady;
    const reason = complete
      ? `Complete recording — runtime captured ${samples.length} scans. Stop: ${String(raw.stop_reason)}.`
      : acquisition.completeness === "complete" && !steady
        ? "Incomplete recording — the sample sequence changes its record period, so an execution is missing."
        : `Incomplete recording — ${String(acquisition.reason)}`;
    return { samples: [], rawSamples, evidence: { ...acquisition, recordingId: this.id, ...(typeof raw.program_revision === "string" ? { programRevision: raw.program_revision } : {}), scanRange: acquisition.scanRange ?? undefined, completeness: complete ? "complete" : "incomplete", reason } };
  }

  /** Validates runtime samples in order: each one's scan and time must increase from the previous one, here or on an earlier page. */
  private decodeSamples(rawSamples: unknown[], previous?: { scan: number; runtimeTimeNs: number }): CapturedSample[] {
    const symbolic = this.bindings.some(binding => binding.symbol);
    const samples = rawSamples as CapturedSample[];
    let previousScan = previous?.scan ?? -1;
    let previousTime = previous?.runtimeTimeNs ?? -1;
    for (const sample of samples) {
      if (!record(sample) || !Number.isSafeInteger(sample.scan) || sample.scan <= previousScan
          || !Number.isFinite(sample.runtime_time_ns) || sample.runtime_time_ns <= previousTime
          || !Number.isFinite(sample.captured_at_ms) || sample.captured_at_ms < 0
          || !Array.isArray(sample.values) || sample.values.length !== this.bindings.length
          || !sample.values.every(value => value === null || typeof value === "number" && Number.isFinite(value))
          || !Array.isArray(sample.forced) || sample.forced.length !== this.bindings.length
          || !sample.forced.every(value => typeof value === "boolean")
          || (symbolic && sample.quality === undefined)
          || (sample.quality !== undefined && (!Array.isArray(sample.quality) || sample.quality.length !== this.bindings.length
            || !sample.quality.every(value => ["good", "bad", "forced", "missing"].includes(value))))) throw new Error("The runtime recording contains invalid samples.");
      if (sample.values.some((value, index) => this.bindings[index].iec_type === "BOOL"
        && value !== null && value !== 0 && value !== 1)) throw new Error("The recording contains an invalid BOOL value.");
      previousScan = sample.scan; previousTime = sample.runtime_time_ns;
    }
    return samples;
  }

  /** Maps samples to the workflow shape. Elapsed time runs from the recording's first sample; the cycle time runs from the
   * previous sample, which for a page's first sample is the last sample of the page before it. */
  private toWorkflowSamples(samples: CapturedSample[], firstTimeNs: number, previousTimeNs?: number): WorkflowResultSample[] {
    const modeAddress = controllerModeAddress(this.loop);
    const modeIndex = this.bindings.findIndex(binding => binding.iec_type === "BOOL" &&
      (modeAddress ? binding.address === modeAddress : Boolean(this.loop.controller.modeSignal)
        && binding.symbol === this.loop.controller.modeSignal));
    const value = (sample: CapturedSample, id: string) => sample.values[this.bindings.findIndex(binding => binding.id === id)] ?? null;
    return samples.map((sample, index) => {
      const modeValue = modeIndex >= 0 && !sample.forced[modeIndex]
        && (sample.quality === undefined || sample.quality[modeIndex] === "good") ? sample.values[modeIndex] : null;
      const whenTrue = this.loop.controller.modeWhenTrue ?? "Auto";
      const previous = index ? samples[index - 1].runtime_time_ns : previousTimeNs;
      return {
        elapsedSeconds: (sample.runtime_time_ns - firstTimeNs) / 1e9,
        observedAtMs: sample.captured_at_ms,
        actualCycleSeconds: previous === undefined ? null : (sample.runtime_time_ns - previous) / 1e9,
        scan: sample.scan, source: this.source, loopRevision: this.loop.revision,
        sp: value(sample, "setpoint"), pv: value(sample, "processVariable"),
        // A declared actuator reports what the device reached; the manipulated variable is then only the request.
        requestedMv: this.loop.signals.actuator ? value(sample, "manipulatedVariable") : null,
        appliedMv: value(sample, this.loop.signals.actuator ? "actuator" : "manipulatedVariable"),
        units: { sp: this.loop.signals.setpoint?.unit ?? "",
          pv: this.loop.signals.processVariable.unit, mv: this.loop.signals.manipulatedVariable.unit },
        quality: sample.forced.some(Boolean) || sample.quality?.some(value => value === "forced") ? "forced"
          : (sample.values.some(value => value === null) || sample.quality?.some(value => value !== "good") || this.loop.signals.additional?.some(binding =>
          binding.role === "quality" && value(sample, binding.id) !== 1)) ? "bad" : "good",
        freshnessMs: null,
        controllerMode: modeValue === 1 ? whenTrue : modeValue === 0 ? whenTrue === "Auto" ? "Manual" : "Auto" : "Unavailable",
        outputLimits: { minimum: this.loop.limits.outputMinimum, maximum: this.loop.limits.outputMaximum },
        rateLimits: { upPerSecond: this.loop.limits.rateUpPerS, downPerSecond: this.loop.limits.rateDownPerS },
        safetyState: "Unavailable", events: [],
        additional: captureAdditionalChannels(this.loop, id => value(sample, id),
          id => sample.forced[this.bindings.findIndex(binding => binding.id === id)] ?? false),
      };
    });
  }
}
function record(value: unknown): value is Record<string, any> { return !!value && typeof value === "object" && !Array.isArray(value); }
