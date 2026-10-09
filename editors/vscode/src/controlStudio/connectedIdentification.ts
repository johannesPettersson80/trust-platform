import { RuntimeControlError } from "../runtimeControlClient";
import { connectedIdentificationChannels } from "./connectedIdentificationChannels";
import { randomUUID } from "crypto";
import type { AutomaticIdentificationResult, IdentificationCampaignRecord } from "./automaticIdentification";
import { BoundedAnalysisJob } from "./boundedAnalysisJob";
import { connectedIdentificationRecord } from "./connectedIdentificationRecord";
import { buildControlExperimentPlan, classifyControlExperimentStatus, ControlExperimentClient, type ControlExperimentPlan } from "./experiment";
import { fitRecordedModel } from "./recordedModelIdentification";
import { identificationAmplitude, identificationMethod, onOffActuator, type IdentificationMethod } from "./identificationPolicy";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { LoopMonitorSample, LoopRecording } from "./loopMonitor";
import { RuntimeRecordingSession, type RecordingOwnerPersistence, type RecordingRecoveryOwner } from "./runtimeRecording";
import type { RuntimeRequest, RuntimeVerificationAttempt, RuntimeVerificationResult } from "./verificationClient";
import { ConnectedIdentificationMode, type IdentificationModePersistence } from "./connectedIdentificationMode";
import { RecordingStartUnconfirmed } from "./recordingStartFailure";
import { readExperimentIdentity, releaseRecording } from "./identificationReconcile";
import { MINIMUM_EXPERIMENT_DURATION_MS } from "./controlStudioLimits";
import {
  campaignMaximumMs, campaignRecordingMs, experimentStopFailure, formatDuration, IdentificationCampaign, requireCampaignLimits, restorationConfirmed,
  systemClock,
  type BaselineSummary, type CampaignClock, type ExperimentState, type IdentificationCampaignPersistence, type RecordingFollower,
} from "./identificationCampaign";

export interface ConnectedIdentificationPorts {
  sample(): Promise<LoopMonitorSample>;
  recordingPersistence?: RecordingOwnerPersistence;
  recordingOwner(): RecordingRecoveryOwner | undefined;
  persist(plan: ControlExperimentPlan, recording: RecordingRecoveryOwner): Promise<void>;
  clear(): Promise<void>;
  retain(recording: LoopRecording): Promise<void>;
  mode: IdentificationModePersistence;
  /** The campaign's phase journal: what reattaching needs after a restart (Specification 35, A8.5). */
  campaign: IdentificationCampaignPersistence;
}

type ModeLike = Pick<ConnectedIdentificationMode, "enter" | "restore" | "processNoise">;
type IdentificationChannel = Awaited<ReturnType<typeof connectedIdentificationChannels>>[number];

export interface ConnectedIdentificationOptions {
  /** Injected so tests can run the supervision rule; the system clock by default. */
  clock?: CampaignClock;
  /** Starts the recording the campaign follows; the runtime's recording session by default. */
  startRecording?(loop: ControlLoopDefinition, persistence: RecordingOwnerPersistence | undefined, plannedDurationMs: number,
    expectedSettlingSeconds?: number): Promise<RecordingFollower>;
  createMode?(loop: ControlLoopDefinition, request: RuntimeRequest, persistence: IdentificationModePersistence): ModeLike;
  fit?: typeof fitRecordedModel;
  /** The output's expected settling time in seconds, when the study declares one or Validate's requirements set it (B3.4 step 2).
   * The recording's record period follows it (A4.2). */
  expectedSettlingSeconds?: number;
}

/** Start refusals before the runtime accepted an experiment: no experiment exists, so nothing is reconciled. */
const REFUSED_START_CODES = ["invalid_experiment", "controller_mode_blocked", "physical_approval_required", "invalid_address",
  "invalid_value", "outside_limits", "io_unavailable", "stale_scan", "forced_signal", "incompatible_binding",
  "restoration_mismatch", "experiment_state_unavailable", "experiment_active"];

/** A released client sends no further request: the runtime keeps its experiment, recording and journal (A8.5, A10.4). */
const RELEASED_REQUEST = "The identification follower was released; the runtime keeps the experiment.";

/** Owns the live campaign and its original recording through restoration. */
export class ConnectedIdentification {
  private readonly fitting = new BoundedAnalysisJob();
  private readonly experiment: ControlExperimentClient;
  private readonly retained: IdentificationCampaignRecord = { status: "running", runs: [] };
  private readonly clock: CampaignClock;
  private cancelled = false;
  private active: ControlExperimentPlan | undefined;
  private attempt: RuntimeVerificationAttempt | undefined;
  private running: Promise<unknown> | undefined;
  private recorder: RecordingFollower | undefined;
  private mode: ModeLike | undefined;
  /** Set when the host releases this client: the runtime-owned experiment, recording and journal are left as they are (A8.5). */
  private detached = false;
  /** False once this client must leave its experiment to the runtime: a reattached or unreconciled start is never aborted here (A8.6, E11.4). */
  private mayAbort = true;
  constructor(private readonly request: RuntimeRequest, private readonly ports: ConnectedIdentificationPorts,
    private readonly options: ConnectedIdentificationOptions = {}) {
    this.experiment = new ControlExperimentClient((operation, params) => this.detached
      ? Promise.reject(new Error(RELEASED_REQUEST)) : request(operation, params as Record<string, unknown>));
    this.clock = options.clock ?? systemClock;
  }

  run(loop: ControlLoopDefinition, progress: (text: string) => Promise<void>): Promise<AutomaticIdentificationResult> {
    const running = this.execute(loop, progress);
    this.running = running;
    return running;
  }
  recordValidation(loop: ControlLoopDefinition, progress: (text: string) => Promise<void>): Promise<RuntimeVerificationResult> {
    const running = this.execute(loop, progress, true);
    this.running = running;
    return running;
  }
  /** Reattaches to the campaign journalled before the host closed: follows its running experiment, finishes it once settled, then fits. */
  resume(loop: ControlLoopDefinition, recorder: RecordingFollower, progress: (text: string) => Promise<void>): Promise<AutomaticIdentificationResult> {
    const running = this.resumeCampaign(loop, recorder, progress);
    this.running = running;
    return running;
  }
  snapshot(): IdentificationCampaignRecord { return structuredClone(this.retained); }
  cancellationOwner() { return { cancel: () => this.cancel(), snapshot: () => {
    const checkpoint = this.ports.mode.read();
    if (checkpoint) return { status: "cancellation-unconfirmed" as const,
      jobId: checkpoint.transfer?.owner.id ?? `mode:${checkpoint.original.program_revision}`,
      projectRevision: checkpoint.original.program_revision, request: { execution: "connected-runtime" },
      reason: "The original controller mode and commands have not been restored." };
    const recording = this.ports.recordingOwner();
    if (recording) return { status: "cancellation-unconfirmed" as const, jobId: recording.recordingId,
      projectRevision: recording.programRevision ?? recording.loopRevision, request: { execution: "connected-runtime" },
      reason: "The original identification recording has not been stopped." };
    return this.attempt && structuredClone(this.attempt);
  } }; }
  /**
   * Closing releases this client only (A8.5, A10.4): the fit stops and the follower refuses its next page, so this client sends no
   * further request. The runtime keeps the experiment and its recording, the journal stays, and reopening reattaches (A8.6).
   * Nothing is aborted or restored here; only an explicit Cancel does that (cancel()).
   */
  dispose(): void { this.detached = true; this.fitting.cancel(); }
  /** True when nothing is left owned here: no live experiment, mode checkpoint, recording or journal (A8.6). */
  isSettled(): boolean {
    return this.active === undefined && this.ports.mode.read() === undefined && this.ports.campaign.read() === undefined
      && this.ports.recordingOwner() === undefined;
  }

  private execute(loop: ControlLoopDefinition, progress: (text: string) => Promise<void>, validationOnly: true): Promise<RuntimeVerificationResult>;
  private execute(loop: ControlLoopDefinition, progress: (text: string) => Promise<void>, validationOnly?: false): Promise<AutomaticIdentificationResult>;
  private async execute(loop: ControlLoopDefinition, progress: (text: string) => Promise<void>, validationOnly = false): Promise<AutomaticIdentificationResult | RuntimeVerificationResult> {
    const sample = await this.ports.sample();
    const method = identificationMethod(loop, sample);
    const needsMode = this.ports.mode.read() || sample.availability === "available" && sample.controller.mode !== (method.manual ? "Manual" : "Auto");
    const mode = this.mode = needsMode && loop.topology !== "open-loop" ? this.createMode(loop) : undefined;
    return this.conclude(mode, async () => {
      await mode?.enter(method.manual);
      this.checkCancelled();
      await progress(validationOnly ? "Checking model — recording the connected plant…" : "Identifying — recording the connected plant…");
      await this.acquire(loop, method, validationOnly, sample, progress);
      if (validationOnly) { this.retained.status = "completed"; return this.retained.runs[0]; }
      return this.fit(loop, progress);
    });
  }

  /** Records the attempt's outcome, and restores the loop's mode once no live experiment is left running (A6.2). */
  private async conclude<T>(mode: ModeLike | undefined, work: () => Promise<T>): Promise<T> {
    try {
      return await work();
    } catch (error) {
      this.retained.status = this.cancelled ? "cancelled" : "failed";
      this.retained.reason = error instanceof Error ? error.message : String(error);
      throw error;
    } finally {
      // A released client leaves a live experiment, its recording and the journal to the runtime; reopening completes them.
      if (!this.detached) await this.settle(mode);
      // Released before any experiment was sent: no runtime owner would return the mode, so it is returned now (A6.2).
      else if (!this.active) await mode?.restore().catch(() => undefined);
    }
  }

  /** Ends the owned live experiment, then keeps the recording and restores the mode once no live experiment is left (A6.2). */
  private async settle(mode: ModeLike | undefined): Promise<void> {
    await this.releaseActive();
    if (!this.active) {
      await this.settleRecording();
      try { await mode?.restore(); }
      catch (error) { this.retained.status = "failed"; this.retained.reason = String(error); throw error; }
    }
  }

  /** The campaign: the recording, the steady baseline, then one step per plant input, each held until it settles and restored. */
  private async acquire(loop: ControlLoopDefinition, method: IdentificationMethod, validation: boolean, sample: LoopMonitorSample,
    progress: (text: string) => Promise<void>): Promise<void> {
    requireCampaignLimits(loop);
    const maximumMs = campaignMaximumMs(loop);
    // Refused before any recording or write (A2.5): a physical target needs the reviewed approval this path does not carry.
    if (sample.availability === "available" && sample.source === "Physical") {
      throw new Error("Live physical identification requires the reviewed physical experiment approval.");
    }
    if (method.kind === "relay" && loop.experiment?.relayDirection === "negative") {
      throw new Error("The connected runtime does not support negative-gain relay feedback.");
    }
    const channels = await connectedIdentificationChannels(loop, this.request);
    const recordingMs = campaignRecordingMs(loop);
    // The record period follows the response (A4.2). Its settling time is not known yet: without a declared one, a response
    // ten times faster than the maximum duration still gets about 200 samples.
    const recorder = await this.startRecording(loop, recordingMs, this.options.expectedSettlingSeconds ?? maximumMs / 10_000);
    await this.ports.campaign.write({ schemaVersion: 1, loopRevision: loop.revision, phase: "baseline",
      recording: recorder.recoveryOwner(), channelsRemaining: channels.length });
    const offer = !!loop.signals.setpoint && sample.availability === "available" && sample.controller.mode === "Auto";
    const campaign = this.campaign(loop, recorder, progress, offer);
    const baseline = await campaign.baseline(maximumMs);
    const baselineEndSeconds = campaign.seconds;
    if (this.mode) this.mode.processNoise = baseline.halfRange;
    const plans: ControlExperimentPlan[] = [];
    for (const [index, channel] of channels.entries()) {
      this.checkCancelled();
      const remaining = channels.length - index;
      const stepSeconds = campaign.seconds;
      // The steps share the maximum duration, counted from the end of the baseline, so a long wait for a steady start never
      // shortens them. Each leaves one baseline window for its restoration and ends within the recording (B3.4).
      const availableMs = Math.min(maximumMs - (stepSeconds - baselineEndSeconds) * 1_000, recordingMs - stepSeconds * 1_000);
      const holdMs = Math.floor(availableMs / remaining) - Math.round(baseline.windowSeconds * 1_000);
      const plan = await this.preparePlan(loop, channel, method, validation, index, channels.length, baseline, holdMs);
      await this.dispatch(loop, recorder, plan, baseline, stepSeconds, remaining - 1);
      const state = await campaign.hold(plan, baseline, stepSeconds);
      if (!restorationConfirmed(state)) throw new Error("The experiment could not restore its original command.");
      this.active = undefined;
      const failure = experimentStopFailure(state);
      if (failure) throw new Error(failure);
      plans.push(plan);
    }
    await this.completeRecording(loop, recorder, plans, Math.max(0, baselineEndSeconds - baseline.windowSeconds), baselineEndSeconds);
  }

  /** Reattaches to a journalled campaign whose experiment is still running (A8.5): follows it, finishes it once settled, restores and fits. */
  private async resumeCampaign(loop: ControlLoopDefinition, recorder: RecordingFollower,
    progress: (text: string) => Promise<void>): Promise<AutomaticIdentificationResult> {
    const journal = this.ports.campaign.read();
    const plan = journal?.plan;
    const baseline = journal?.baseline;
    if (!journal || !plan || !baseline || journal.phase !== "step" || journal.loopRevision !== loop.revision) {
      throw new Error("The interrupted identification has no running experiment to resume. Identify again to start a new campaign.");
    }
    this.recorder = recorder;
    this.active = plan;
    this.mayAbort = false; // Reopening never aborts a running experiment: a failure leaves it to the runtime (A8.6).
    this.attempt = { status: "cancellation-unconfirmed", jobId: plan.id, projectRevision: recorder.recoveryOwner().programRevision ?? loop.revision,
      request: { execution: "connected-runtime", experiment: plan }, reason: "Live experiment restoration is pending." };
    const mode = this.mode = this.createMode(loop);
    await this.conclude(mode, async () => {
      const identity = await readExperimentIdentity(id => this.experiment.status(id), plan.id);
      if (identity.kind === "unresolved") throw new Error(`${identity.reason} The experiment is left to the runtime; reopening reconciles it.`);
      if (identity.kind === "absent") {
        this.active = undefined;
        throw new Error(`The interrupted experiment never started, so nothing was applied and the original command is restored. ${identity.reason}`);
      }
      const state = await this.campaign(loop, recorder, progress, false).hold(plan, baseline, journal.stepStartSeconds ?? 0);
      if (!restorationConfirmed(state)) throw new Error("The experiment could not restore its original command.");
      this.active = undefined;
      const failure = experimentStopFailure(state);
      if (failure) throw new Error(failure);
      if (journal.channelsRemaining > 0) {
        throw new Error(`The interrupted identification covered one of ${journal.channelsRemaining + 1} plant inputs. Identify again to cover the others.`);
      }
      const stepStart = journal.stepStartSeconds ?? 0;
      await this.completeRecording(loop, recorder, [plan], Math.max(0, stepStart - baseline.windowSeconds), stepStart);
    });
    // The mode has returned by now, once the experiment has ended; the fit follows (A6.2, A8.6).
    return this.fit(loop, progress);
  }

  private async preparePlan(loop: ControlLoopDefinition, channel: IdentificationChannel, method: IdentificationMethod, validation: boolean,
    index: number, channelCount: number, baseline: BaselineSummary, holdMs: number): Promise<ControlExperimentPlan> {
    if (holdMs < MINIMUM_EXPERIMENT_DURATION_MS) {
      throw new Error(`The maximum experiment duration leaves no time to hold the step after the baseline and its restoration (${formatDuration(Math.max(0, holdMs) / 1_000)} free). Open Limits… for a longer maximum duration.`);
    }
    const sample = channel.sample(await this.ports.sample());
    const amplitude = identificationAmplitude({ loop: channel.loop, sample, kind: method.kind, automatic: method.automatic,
      noise: baseline.noise, validation, fallbackAmplitude: (loop.limits.outputMaximum - loop.limits.outputMinimum) * 0.1,
      onOff: onOffActuator(loop) });
    const prepared = buildControlExperimentPlan({ loop: channel.loop, sample, kind: method.kind, amplitude, durationSeconds: holdMs / 1_000,
      seed: (loop.experiment?.seed ?? 1) + (validation ? 1 : 0) + index * 7919,
      ...(channelCount === 1 ? { processCenter: baseline.mean, processNoise: baseline.halfRange } : {}) });
    if (prepared.kind !== "accepted") throw new Error(prepared.reason);
    if (prepared.plan.source !== "simulation") throw new Error("Live physical identification requires the reviewed physical experiment approval.");
    return { ...prepared.plan, id: randomUUID() };
  }

  /** Journals the step and its experiment before the start is sent. The start is sent once; an uncertain acknowledgement is reconciled, never resent. */
  private async dispatch(loop: ControlLoopDefinition, recorder: RecordingFollower, plan: ControlExperimentPlan, baseline: BaselineSummary,
    stepSeconds: number, channelsRemaining: number): Promise<void> {
    const owner = recorder.recoveryOwner();
    await this.ports.campaign.write({ schemaVersion: 1, loopRevision: loop.revision, phase: "step", recording: owner, plan, baseline,
      stepStartSeconds: stepSeconds, channelsRemaining });
    await this.ports.persist(plan, owner);
    this.checkCancelled();
    this.active = plan;
    this.attempt = { status: "cancellation-unconfirmed", jobId: plan.id, projectRevision: owner.programRevision ?? loop.revision,
      request: { execution: "connected-runtime", experiment: plan }, reason: "Live experiment restoration is pending." };
    try { await this.experiment.start(plan, false); }
    catch (error) {
      if (error instanceof RuntimeControlError && REFUSED_START_CODES.includes(error.code ?? "")) {
        this.active = undefined;
        if (this.attempt) this.attempt = { ...this.attempt, status: "failed", reason: error.message };
        throw error;
      }
      await this.reconcileStart(plan, error);
    }
  }

  /** A start whose outcome is unknown is reconciled by its identity and never sent again (E11.4, A8.5). */
  private async reconcileStart(plan: ControlExperimentPlan, startError: unknown): Promise<void> {
    const identity = await readExperimentIdentity(id => this.experiment.status(id), plan.id);
    // Running or ended: acquire follows the experiment to its end, as a resumed campaign does.
    if (identity.kind === "running" || identity.kind === "terminal") return;
    if (identity.kind === "absent") {
      this.active = undefined;
      if (this.attempt) this.attempt = { ...this.attempt, status: "failed", reason: "The experiment never started; nothing was applied." };
      throw new Error(`The experiment start was not confirmed, and the runtime holds no experiment with its identity, so none ran. ${identity.reason}`);
    }
    // Unreconciled: the runtime's experiment, recording, journal and mode stay owned here until reopening reconciles them.
    this.mayAbort = false;
    throw new Error(`The experiment start was not confirmed and its status cannot be read. ${identity.reason} The start is not sent again.`);
  }

  private async startRecording(loop: ControlLoopDefinition, plannedDurationMs: number, expectedSettlingSeconds: number): Promise<RecordingFollower> {
    const start: NonNullable<ConnectedIdentificationOptions["startRecording"]> = this.options.startRecording
      ?? ((target: ControlLoopDefinition, persistence: RecordingOwnerPersistence | undefined, duration: number, settling?: number) =>
        RuntimeRecordingSession.start(target, persistence, duration, settling));
    try {
      this.recorder = await start(loop, this.ports.recordingPersistence, plannedDurationMs, expectedSettlingSeconds);
      return this.recorder;
    } catch (error) {
      if (error instanceof RecordingStartUnconfirmed && error.session instanceof RuntimeRecordingSession) this.recorder = error.session;
      throw error;
    }
  }

  /** Stops and keeps the recording, then builds the run from it. Only complete evidence can become a run (connectedIdentificationRecord). */
  /** Retains the whole recording; the fit uses it from `fitFromSeconds`, the start of the steady baseline window, and takes the
   * operating point over that window, which ends at `baselineEndSeconds` (B3.4, N1.3). */
  private async completeRecording(loop: ControlLoopDefinition, recorder: RecordingFollower, plans: readonly ControlExperimentPlan[],
    fitFromSeconds: number, baselineEndSeconds: number): Promise<void> {
    const recording = await this.retainRecording(recorder);
    const result = connectedIdentificationRecord(loop, recording, plans[0], fitFromSeconds, baselineEndSeconds);
    result.request = { ...result.request, experiments: [...plans] };
    this.retained.runs = [...this.retained.runs, result];
    this.attempt = { ...this.attempt!, status: "completed", reason: "Connected experiment and restoration completed.", run: result.run };
    if (result.evaluation.status !== "Pass") throw new Error("The measured experiment exceeded its signal limits or has missing samples.");
  }

  private async fit(loop: ControlLoopDefinition, progress: (text: string) => Promise<void>): Promise<AutomaticIdentificationResult> {
    this.checkCancelled();
    await progress("Fitting the measured response…");
    const result = await (this.options.fit ?? fitRecordedModel)(loop, this.retained.runs, this.fitting, () => this.cancelled);
    this.retained.status = "completed";
    return result;
  }

  /** Ends a live experiment still owned here through the runtime's restoration path, and keeps it owned if restoration is unconfirmed. */
  private async releaseActive(): Promise<void> {
    if (!this.active || !this.mayAbort) return;
    try {
      await this.experiment.abort(this.active.id);
      if (await this.confirmRestored(this.active.id)) this.active = undefined;
    } catch { /* The persisted experiment and buffer stay owned here; a start is never replayed. */ }
  }

  /** Keeps the recording and clears the journal once no live experiment is left. */
  private async settleRecording(): Promise<void> {
    if (this.recorder) await this.retainRecording(this.recorder);
    if (this.attempt?.status === "cancellation-unconfirmed") {
      this.attempt = { ...this.attempt, status: this.cancelled ? "cancelled" : "failed", reason: "Original command restored." };
    }
    await this.ports.clear();
  }

  /** Polls until the experiment has ended and its original command is restored, within the bound of the abort path. */
  private async confirmRestored(experimentId: string): Promise<boolean> {
    for (let retry = 0; retry < 50; retry += 1) {
      const state = await this.experiment.status(experimentId) as ExperimentState;
      if (classifyControlExperimentStatus(state) === "terminal" && restorationConfirmed(state)) return true;
      await this.clock.wait(100);
    }
    return false;
  }

  private campaign(loop: ControlLoopDefinition, recorder: RecordingFollower, progress: (text: string) => Promise<void>,
    offerSetpointStep: boolean): IdentificationCampaign {
    return new IdentificationCampaign({ loop, recorder: this.followedBy(recorder), experiment: this.experiment, clock: this.clock, progress,
      cancelled: () => this.cancelled, offerSetpointStep, expectedSettlingSeconds: this.options.expectedSettlingSeconds });
  }

  /** Once released, the follower refuses its next page: the campaign's poll ends here and sends nothing more to the runtime. */
  private followedBy(recorder: RecordingFollower): RecordingFollower {
    return {
      follow: () => this.detached
        ? Promise.reject(new Error("The identification follower was released; the runtime keeps the experiment."))
        : recorder.follow(),
      stop: () => recorder.stop(),
      recoveryOwner: () => recorder.recoveryOwner(),
    };
  }

  /** Keeps the recording's samples, then releases the runtime buffer: a failed keep leaves the buffer for a retry (E4.7, E11.6). */
  private async retainRecording(recorder: RecordingFollower): Promise<LoopRecording> {
    const recording = await recorder.stop();
    await this.ports.retain(recording);
    await releaseRecording(recorder);
    this.recorder = undefined;
    return recording;
  }

  private createMode(loop: ControlLoopDefinition): ModeLike {
    return this.options.createMode?.(loop, this.request, this.ports.mode) ?? new ConnectedIdentificationMode(loop, this.request, this.ports.mode);
  }

  private checkCancelled(): void { if (this.cancelled) throw new Error("Identification cancelled."); }

  private async cancel(): Promise<void> {
    this.cancelled = true; this.fitting.cancel();
    await this.running?.catch(() => undefined);
    if (this.active) {
      await this.experiment.abort(this.active.id);
      if (!(await this.confirmRestored(this.active.id))) throw new Error("Reconnect the original runtime to confirm experiment restoration.");
      if (this.recorder) await this.retainRecording(this.recorder);
      await this.ports.clear(); this.active = undefined;
      if (this.attempt) this.attempt = { ...this.attempt, status: "cancelled", reason: "Original command restored." };
    }
    await this.mode?.restore();
    if (this.recorder) { await this.retainRecording(this.recorder); await this.ports.clear(); }
  }
}
