import { RuntimeControlError } from "../runtimeControlClient";
import { connectedIdentificationChannels } from "./connectedIdentificationChannels";
import { randomUUID } from "crypto";
import type { AutomaticIdentificationResult, IdentificationCampaignRecord } from "./automaticIdentification";
import { BoundedAnalysisJob } from "./boundedAnalysisJob";
import { connectedIdentificationRecord } from "./connectedIdentificationRecord";
import { buildControlExperimentPlan, classifyControlExperimentStatus, ControlExperimentClient, type ControlExperimentPlan } from "./experiment";
import { fitRecordedModel } from "./recordedModelIdentification";
import { identificationLoop } from "./identificationPolicy";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { LoopMonitorSample, LoopRecording } from "./loopMonitor";
import { RuntimeRecordingSession, type RecordingOwnerPersistence, type RecordingRecoveryOwner } from "./runtimeRecording";
import type { RuntimeRequest, RuntimeVerificationAttempt, RuntimeVerificationResult } from "./verificationClient";
import { ConnectedIdentificationMode, type IdentificationModePersistence } from "./connectedIdentificationMode";
import { RecordingStartUnconfirmed } from "./recordingStartFailure";

export interface ConnectedIdentificationPorts {
  sample(): Promise<LoopMonitorSample>;
  recordingPersistence?: RecordingOwnerPersistence;
  recordingOwner(): RecordingRecoveryOwner | undefined;
  persist(plan: ControlExperimentPlan, recording: RecordingRecoveryOwner): Promise<void>;
  clear(): Promise<void>;
  retain(recording: LoopRecording): Promise<void>;
  mode: IdentificationModePersistence;
}

/** Owns the live experiment and its original recording through restoration. */
export class ConnectedIdentification {
  private readonly fitting = new BoundedAnalysisJob();
  private readonly experiment: ControlExperimentClient;
  private readonly retained: IdentificationCampaignRecord = { status: "running", runs: [] };
  private cancelled = false;
  private active: ControlExperimentPlan | undefined;
  private attempt: RuntimeVerificationAttempt | undefined;
  private running: Promise<unknown> | undefined;
  private recorder: RuntimeRecordingSession | undefined;
  private mode: ConnectedIdentificationMode | undefined;
  constructor(private readonly request: RuntimeRequest, private readonly ports: ConnectedIdentificationPorts) {
    this.experiment = new ControlExperimentClient((operation, params) => request(operation, params as Record<string, unknown>));
  }

  run(loop: ControlLoopDefinition, progress: (text: string) => Promise<void>): Promise<AutomaticIdentificationResult> {
    const running = this.execute(identificationLoop(loop), progress);
    this.running = running;
    return running;
  }
  recordValidation(loop: ControlLoopDefinition, progress: (text: string) => Promise<void>): Promise<RuntimeVerificationResult> {
    const running = this.execute(identificationLoop(loop), progress, true);
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
  dispose(): void { this.fitting.cancel(); if (this.retained.status === "running") void this.cancel().catch(() => undefined); }

  private execute(loop: ControlLoopDefinition, progress: (text: string) => Promise<void>, validationOnly: true): Promise<RuntimeVerificationResult>;
  private execute(loop: ControlLoopDefinition, progress: (text: string) => Promise<void>, validationOnly?: false): Promise<AutomaticIdentificationResult>;
  private async execute(loop: ControlLoopDefinition, progress: (text: string) => Promise<void>, validationOnly = false): Promise<AutomaticIdentificationResult | RuntimeVerificationResult> {
    const sample = await this.ports.sample();
    const manual = loop.experiment?.kind !== "setpoint_step";
    const needsMode = this.ports.mode.read() || sample.availability === "available" && sample.controller.mode !== (manual ? "Manual" : "Auto");
    const mode = this.mode = needsMode && loop.topology !== "open-loop" ? new ConnectedIdentificationMode(loop, this.request, this.ports.mode) : undefined;
    try {
      await mode?.enter(manual);
      this.checkCancelled();
      await progress(validationOnly ? "Checking model — recording the connected plant…" : "Identifying — recording the connected plant…");
      await this.acquire(loop, validationOnly ? 1 : 0, progress);
      if (validationOnly) { this.retained.status = "completed"; return this.retained.runs[0]; }
      this.checkCancelled();
      await progress("Fitting the measured response…");
      const result = await fitRecordedModel(loop, this.retained.runs, this.fitting, () => this.cancelled);
      this.retained.status = "completed";
      return result;
    } catch (error) {
      this.retained.status = this.cancelled ? "cancelled" : "failed";
      this.retained.reason = error instanceof Error ? error.message : String(error);
      throw error;
    } finally {
      if (!this.active) {
        try { await mode?.restore(); }
        catch (error) { this.retained.status = "failed"; this.retained.reason = String(error); throw error; }
      }
    }
  }

  private async acquire(loop: ControlLoopDefinition, trial: number, progress: (text: string) => Promise<void>): Promise<void> {
    const duration = loop.experiment?.durationSeconds ?? 30;
    const baselineSeconds = Math.max(loop.controller.taskPeriodMs * 3 / 1000, duration * 0.1);
    const channels = await connectedIdentificationChannels(loop, this.request);
    const sample = await this.ports.sample();
    const configuredAmplitude = loop.experiment?.amplitude || (loop.limits.outputMaximum - loop.limits.outputMinimum) * 0.1;
    const options = { loop, sample, kind: loop.experiment?.kind ?? "automatic" as const,
      amplitude: configuredAmplitude * (trial ? 0.75 : 1), durationSeconds: (duration - baselineSeconds) / channels.length,
      seed: (loop.experiment?.seed ?? 1) + trial };
    // Review before acquiring or writing. The runtime repeats bounds, scan and restoration checks.
    const review = buildControlExperimentPlan(options);
    if (review.kind !== "accepted") throw new Error(review.reason);
    if (review.plan.source !== "simulation") throw new Error("Live physical identification requires the reviewed physical experiment approval.");
    if (review.plan.kind === "relay" && loop.experiment?.relayDirection === "negative") throw new Error("The connected runtime does not support negative-gain relay feedback.");
    let recorder: RuntimeRecordingSession | undefined;
    let terminal = false;
    await this.awaitSteadyOutput(loop, progress);
    try {
      try { recorder = this.recorder = await RuntimeRecordingSession.start(loop, this.ports.recordingPersistence); }
      catch (error) {
        if (error instanceof RecordingStartUnconfirmed && error.session instanceof RuntimeRecordingSession) recorder = this.recorder = error.session;
        throw error;
      }
      const baseline = await this.observeBaseline(baselineSeconds * 1000);
      const plans: ControlExperimentPlan[] = [];
      for (const [index, channel] of channels.entries()) {
      this.checkCancelled(); terminal = false;
      const prepared = buildControlExperimentPlan({ ...options, loop: channel.loop,
        amplitude: (channel.loop.experiment?.amplitude || configuredAmplitude) * (trial ? 0.75 : 1),
        sample: channel.sample(await this.ports.sample()), seed: options.seed + index * 7919, ...(channels.length === 1 ? baseline : {}) });
      if (prepared.kind !== "accepted") throw new Error(prepared.reason);
      const plan = { ...prepared.plan, id: randomUUID() };
      await this.ports.persist(plan, recorder.recoveryOwner());
      this.checkCancelled();
      this.active = plan;
      this.attempt = { status: "cancellation-unconfirmed", jobId: plan.id,
        projectRevision: recorder.recoveryOwner().programRevision ?? loop.revision,
        request: { execution: "connected-runtime", experiment: plan }, reason: "Live experiment restoration is pending." };
      // Submit once. An uncertain acknowledgement is reconciled against this same ID.
      try { await this.experiment.start(plan, false); }
      catch (error) {
        if (error instanceof RuntimeControlError && ["invalid_experiment", "controller_mode_blocked", "physical_approval_required", "invalid_address", "invalid_value", "outside_limits", "io_unavailable", "stale_scan", "forced_signal", "incompatible_binding", "restoration_mismatch", "experiment_state_unavailable", "experiment_active"].includes(error.code ?? "")) {
          this.active = undefined;
          if (this.attempt) this.attempt = { ...this.attempt, status: "failed", reason: error.message };
          throw error;
        }
        const state = await this.experiment.status(plan.id).catch(() => undefined);
        if (!state || classifyControlExperimentStatus(state) === "invalid") throw error;
      }
      const deadline = Date.now() + plan.durationMs + 10000;
      while (Date.now() < deadline) {
        if (this.cancelled) await this.experiment.abort(plan.id);
        const state = await this.experiment.status(plan.id) as { status: string; restoration: string; stop_reason?: string };
        if (classifyControlExperimentStatus(state) === "terminal") {
          if (!["restored", "not_required"].includes(state.restoration)) throw new Error("The experiment could not restore its original command.");
          terminal = true;
          if (state.status !== "completed") throw new Error(state.stop_reason ?? "Identification cancelled.");
          break;
        }
        await new Promise(resolve => setTimeout(resolve, 100));
      }
      if (!terminal) throw new Error("The live experiment did not confirm completion and restoration.");
      plans.push(plan);
      }
      const plan = plans[0];
      const recording = await recorder.stop(); recorder = this.recorder = undefined;
      await this.ports.retain(recording);
      const result = connectedIdentificationRecord(loop, recording, plan);
      result.request = { ...result.request, experiments: plans };
      this.retained.runs = [...this.retained.runs, result];
      this.attempt = { ...this.attempt!, status: "completed", reason: "Connected experiment and restoration completed.", run: result.run };
      if (result.evaluation.status !== "Pass") throw new Error("The measured experiment exceeded its signal limits or has missing samples.");
    } finally {
      if (this.active && !terminal) {
        try {
          await this.experiment.abort(this.active.id);
          for (let retry = 0; retry < 50; retry++) {
            const state = await this.experiment.status(this.active.id) as { restoration: string };
            if (classifyControlExperimentStatus(state) === "terminal" && ["restored", "not_required"].includes(state.restoration)) { terminal = true; break; }
            await new Promise(resolve => setTimeout(resolve, 100));
          }
        } catch { /* Persisted native experiment and buffer remain owned; never restart them. */ }
      }
      if (!this.active || terminal) {
        if (recorder) { await this.ports.retain(await recorder.stop()); this.recorder = undefined; }
        if (this.attempt?.status === "cancellation-unconfirmed") this.attempt = { ...this.attempt, status: this.cancelled ? "cancelled" : "failed", reason: "Original command restored." };
        await this.ports.clear(); this.active = undefined;
      }
    }
  }

  private checkCancelled(): void { if (this.cancelled) throw new Error("Identification cancelled."); }
  /** A step or excitation starts from a steady output (complete workflow, step identification). Over one observation
   * window the output's drift, the difference between the means of its first and last fifths, must stay within four
   * times its sample-to-sample noise, and at least 0.001 % of its declared range: a residual decay would otherwise
   * become the fitted operating point that validation and Apply start from. While it still moves, wait, bounded. */
  private async awaitSteadyOutput(loop: ControlLoopDefinition, progress: (text: string) => Promise<void>): Promise<void> {
    const output = loop.signals.processVariable;
    const windowMs = Math.max(2000, loop.controller.taskPeriodMs * 20);
    const floor = 1e-5 * (output.range.maximum - output.range.minimum);
    const until = Date.now() + Math.max(60_000, (loop.experiment?.durationSeconds ?? 30) * 3000);
    const window: { at: number; value: number }[] = [];
    let announced = false;
    for (;;) {
      this.checkCancelled();
      const sample = await this.ports.sample().catch(() => undefined);
      const now = Date.now();
      if (sample?.availability === "available" && Number.isFinite(sample.processVariable.value)) window.push({ at: now, value: sample.processVariable.value });
      while (window.length > 1 && window[1].at <= now - windowMs) window.shift();
      if (window.length >= 5 && now - window[0].at >= windowMs) {
        const values = window.map(point => point.value), fifth = Math.max(1, Math.floor(values.length / 5));
        const mean = (part: number[]) => part.reduce((sum, value) => sum + value, 0) / part.length;
        const drift = Math.abs(mean(values.slice(-fifth)) - mean(values.slice(0, fifth)));
        const steps = values.slice(1).map((value, index) => value - values[index]);
        const noise = Math.sqrt(mean(steps.map(step => step * step)) / 2);
        if (drift <= Math.max(4 * noise, floor)) return;
        if (!announced) { announced = true; await progress(`Waiting for ${output.symbol} to settle…`); }
      }
      if (now > until) throw new Error(`${output.symbol} did not settle before identification. Wait for a steady output or choose another permitted test.`);
      await new Promise(resolve => setTimeout(resolve, 100));
    }
  }
  /** Waits out the steady baseline, noting each scan's process value: its mean and half-range centre and size a relay's band. */
  private async observeBaseline(ms: number): Promise<{ processCenter?: number; processNoise?: number }> {
    const values = new Map<number, number>();
    const until = Date.now() + ms;
    while (Date.now() < until) {
      this.checkCancelled();
      const sample = await this.ports.sample().catch(() => undefined);
      if (sample?.availability === "available" && sample.processVariable.quality === "good" && Number.isFinite(sample.processVariable.value)) {
        values.set(sample.scan, sample.processVariable.value);
      }
      await new Promise(resolve => setTimeout(resolve, Math.max(0, Math.min(100, until - Date.now()))));
    }
    const measured = [...values.values()];
    if (measured.length < 10) return {};
    return { processCenter: measured.reduce((sum, value) => sum + value, 0) / measured.length,
      processNoise: (Math.max(...measured) - Math.min(...measured)) / 2 };
  }
  private async cancel(): Promise<void> {
    this.cancelled = true; this.fitting.cancel();
    await this.running?.catch(() => undefined);
    if (this.active) {
      await this.experiment.abort(this.active.id);
      let restored = false;
      for (let retry = 0; retry < 50; retry++) {
        const state = await this.experiment.status(this.active.id) as { restoration: string };
        if (classifyControlExperimentStatus(state) === "terminal" && ["restored", "not_required"].includes(state.restoration)) { restored = true; break; }
        await new Promise(resolve => setTimeout(resolve, 100));
      }
      if (!restored) throw new Error("Reconnect the original runtime to confirm experiment restoration.");
      if (this.recorder) { await this.ports.retain(await this.recorder.stop()); this.recorder = undefined; }
      await this.ports.clear(); this.active = undefined;
      if (this.attempt) this.attempt = { ...this.attempt, status: "cancelled", reason: "Original command restored." };
    }
    await this.mode?.restore();
    if (this.recorder) { await this.ports.retain(await this.recorder.stop()); this.recorder = undefined; await this.ports.clear(); }
  }
}
