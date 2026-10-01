import { awaitRuntimeDuration, runtimeObservationBoundMs, transferClock } from "./runtimeDuration";
import { transferOutputBasis, transferTargetAllowance } from "./transferActuatorFeedback";
import type { ParameterOwner } from "./parameterApplication";
import { CommissioningJournal, type CommissioningCheckpointWriter } from "./commissioningCheckpoint";
import { commissioningObservation, commissioningReferenceTargets } from "./commissioningObservation";
import { connectedModelChannels } from "./loopChannels";
import type { CommissioningProfile } from "./commissioningProfile";
import { formatEngineeringNumber } from "./chartContract";
import { assessCommissioningRecording, type CommissioningCriteria } from "./commissioningAssessment";
import type { CommissioningResult } from "./proportionalCommissioning";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import type { GeneralModel } from "./generalModel";
import type { ControlLoopDefinition } from "./loopDefinition";
import { bindRuntimeConnection } from "./runtimeConnection";
import { RuntimeRecordingSession } from "./runtimeRecording";
import { RuntimeTransferClient, type TransferStatus } from "./runtimeTransfer";
import { connectRuntimeParameters } from "./runtimeParameterConnection";
import type { ParameterApplicationClient, ParameterReview, ParameterStatus } from "./parameterApplication";
import { sameParameterValues } from "./parameterApplication";
import { parallelPidTracking, pidTransferBinding, pidTransferStates } from "./pidTransfer";
import { transferTolerance } from "./proportionalTransfer";
import { anchorIntegratingOutputs, assertCommissioningStartingPoint, assertReferenceHeld, autoEntryTolerance, awaitCommissioningStartingPoint, withReferenceHoldGuard, type StartingPointWait } from "./commissioningStartingPoint";
import type { LoopRecording } from "./loopMonitor";
import { RecordingStartUnconfirmed } from "./recordingStartFailure";
import type { RecordingSession } from "./recordingCoordinator";

const pidProfile: CommissioningProfile = { label: "PID", trackingKind: "parallel-pid-v1",
  binding: pidTransferBinding, states: pidTransferStates, tracking: parallelPidTracking };

/** Shared reviewed stateful transfer lifecycle; numerical/controller laws stay in their adapters. */
export class PidCommissioning {
  private previousReferences: Record<string, number> = {};
  private reviewedMode: boolean | undefined;
  private appliedParameterRevision: string | undefined;
  private recoveryValues: Readonly<Record<string, number>> | undefined;
  private parameterOwner: ParameterOwner | undefined;
  private journal: CommissioningJournal | undefined;
  private cancelled = false;
  private running: Promise<CommissioningResult> | undefined;
  private finished: CommissioningResult | undefined;
  private recording: RecordingSession | undefined;
  private parameterApplicationAttempted = false;
  private clock: (() => Promise<number>) | undefined;
  recoveryConfirmed = true;

  private constructor(private readonly loop: ControlLoopDefinition, private readonly model: GeneralModel,
    private readonly task: EngineerRequirements, private readonly parameters: ParameterApplicationClient,
    private readonly review: ParameterReview, private readonly transfer: RuntimeTransferClient,
    private readonly previousReference: number, private readonly profile: CommissioningProfile) {}

  static async review(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
    parameters: ParameterApplicationClient, review: ParameterReview, profile: CommissioningProfile = pidProfile,
    wait?: StartingPointWait): Promise<PidCommissioning> {
    const connection = await bindRuntimeConnection(loop.runtimeId);
    if (connection.source !== "simulation") throw new Error("This commissioning adapter requires the local simulator.");
    const binding = withReferenceHoldGuard(loop, profile.binding(loop));
    const transfer = new RuntimeTransferClient(binding, review.observed.program_revision, connection.request);
    const first = await transfer.status();
    if (![profile.trackingKind, ...(profile.requiredTracking ?? [])].every(kind => first.supported_tracking?.includes(kind))) throw new Error(`The runtime needs ${profile.trackingKind} state tracking before Auto commissioning.`);
    assertReferenceHeld(loop, first.values);
    const [inputs, outputs] = startingPointSymbols(loop, model);
    model = anchorIntegratingOutputs(model, first.values, outputs);
    const observed = wait ? await awaitCommissioningStartingPoint(loop, model, task, inputs, outputs, () => transfer.status(), wait) : first;
    if (observed.program_revision !== review.observed.program_revision) throw new Error("The program changed during PID commissioning review.");
    const result = new PidCommissioning(loop, structuredClone(model), { ...task }, parameters, review, transfer,
      number(observed, binding.states[0].symbol), profile);
    result.previousReferences = Object.fromEntries(result.referenceSymbols().map(symbol => [symbol, number(observed, symbol)]));
    result.reviewedMode = observed.values[binding.mode.symbol] as boolean;
    result.checkStartingPoint(observed);
    result.clock = transferClock(transfer);
    return result;
  }

  static async reviewRecovery(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
    parameters: ParameterApplicationClient, review: ParameterReview, report: CommissioningResult, profile: CommissioningProfile = pidProfile): Promise<PidCommissioning> {
    PidCommissioning.assertRecoveryNeeded(loop, report, profile);
    const first = report.transfers[0];
    const saved = report.parameters;
    if (model.revision !== report.modelRevision || !first?.transaction?.initial_values || !saved
      || review.observed.program_revision !== saved.program_revision || review.observed.binding_revision !== saved.binding_revision
      || review.observed.active_revision !== saved.active_revision || !sameParameterValues(review.observed.values, saved.values)
      || !saved.previous_values || !sameParameterValues(review.values, saved.previous_values)) {
      throw new Error("The saved controller campaign does not match the reviewed runtime and rollback values.");
    }
    const connection = await bindRuntimeConnection(loop.runtimeId);
    if (connection.source !== "simulation") throw new Error("Controller recovery requires the reviewed simulator.");
    const binding = profile.binding(loop);
    const transfer = new RuntimeTransferClient(binding, review.observed.program_revision, connection.request);
    const current = await transfer.status();
    const reference = first.transaction.initial_values[binding.states[0].symbol];
    if (typeof reference !== "number" || !Number.isFinite(reference) || current.program_revision !== first.program_revision
      || current.binding_revision !== first.binding_revision || current.status === "pending") throw new Error("Controller transfer identity changed or its result remains pending.");
    const result = new PidCommissioning(loop, structuredClone(model), { ...task }, parameters, review, transfer, reference, profile);
    result.previousReferences = Object.fromEntries(result.referenceSymbols().map(symbol => {
      const value = first.transaction!.initial_values![symbol];
      if (typeof value !== "number" || !Number.isFinite(value)) throw new Error(`Original commissioning reference ${symbol} is unavailable.`);
      return [symbol, value];
    }));
    result.recoveryValues = { ...review.values };
    result.parameterApplicationAttempted = true;
    result.finished = structuredClone(report);
    result.recoveryConfirmed = report.recoveryConfirmed;
    result.transfer.outcomes.push(...structuredClone(report.transfers));
    return result;
  }

  description(): string {
    return `Apply the reviewed ${this.profile.label} settings and start Auto at ${formatEngineeringNumber(this.target())} ${this.loop.signals.processVariable.unit}. Monitor for ${this.task.durationSeconds} s.`;
  }

  rollbackDescription(): string {
    return `Return to Manual at the current applied output, restore reference ${formatEngineeringNumber(this.previousReference)} ${this.loop.signals.processVariable.unit}, then restore the previous ${this.profile.label} record. Confirm recovery on the reviewed runtime.`;
  }

  setCheckpointWriter(writer: CommissioningCheckpointWriter): void {
    if (this.running) throw new Error("Install commissioning persistence before starting the campaign.");
    if (!this.review.transaction_id || this.review.observed.supports_request_id !== true) throw new Error("Reload the runtime with recoverable parameter operations before commissioning.");
    this.parameterOwner = { id: this.review.transaction_id, program: this.review.observed.program_revision, binding: this.review.observed.binding_revision };
    this.journal = new CommissioningJournal(this.loop, { model: this.model, task: this.task,
      origin: this.review.observed, candidate: this.review.values,
      parameterOperation: { id: this.review.transaction_id, program: this.review.observed.program_revision, binding: this.review.observed.binding_revision }, references: this.previousReferences,
      binding: this.transfer.binding, recoveryTracking: this.profile.tracking(this.loop, this.transfer.binding, false), tolerance: transferTolerance(this.loop, this.task.transferTolerance) }, writer);
    this.transfer.setOwnerPersistence(owner => this.journal!.save("transfer", { transfer: owner }));
  }

  run(): Promise<CommissioningResult> {
    if (this.running || this.finished) throw new Error("The reviewed controller commissioning campaign has already started.");
    this.running = this.execute().then(result => { this.finished = result; return result; });
    return this.running;
  }

  async cancel(): Promise<CommissioningResult | undefined> {
    // Closing a completed campaign is not a rollback. Preserve its completed
    // journal and receipt, and do not issue another runtime operation.
    if (this.finished?.status === "passed" && this.finished.recoveryConfirmed) return this.finished;
    this.cancelled = true;
    await this.transfer.cancel();
    if (this.finished) {
      if (!this.finished.recoveryConfirmed) {
        try {
          await this.recover();
          const recording = this.recording ? await this.recording.stop() : this.finished.recording;
          this.finished = { ...this.finished, ...(recording ? { recording } : {}), parameters: await this.parameters.status(),
            transfers: this.transfer.outcomes, recoveryConfirmed: true, reason: "Controller recovery and recording stop confirmed in Manual." };
        } catch (error) {
          this.recoveryConfirmed = false;
          this.finished = { ...this.finished, recoveryConfirmed: false, reason: `Controller recovery remains unresolved: ${String(error)}` };
        }
      }
      if (this.finished.recoveryConfirmed) await this.journal?.save("recovered", { recordingResult: this.finished.recording });
      return this.finished;
    }
    if (this.running) return this.running;
    return undefined;
  }

  recordedOutcome(): CommissioningResult | undefined { return this.finished; }
  isCampaignRunning(): boolean { return this.running !== undefined && this.finished === undefined; }

  async settleForClose(): Promise<CommissioningResult | undefined> { return this.cancel(); }

  async rollback(): Promise<ParameterStatus> {
    await this.recover();
    const parameters = await this.parameters.status();
    if (this.finished) this.finished = { ...this.finished, parameters, transfers: this.transfer.outcomes,
      recoveryConfirmed: true, reason: "Controller recovery confirmed in Manual; previous parameters restored." };
    if (this.recording) await this.recording.stop();
    await this.journal?.save("recovered");
    return parameters;
  }

  static assertRecoveryNeeded(loop: ControlLoopDefinition, report: CommissioningResult, profile: CommissioningProfile = pidProfile): void {
    const binding = profile.binding(loop);
    const last = report.transfers[report.transfers.length - 1]?.transaction;
    if (report.recoveryConfirmed !== false && last?.confirmation === "output-image" && last.observed_values?.[binding.mode.symbol] === binding.mode.manual_value) {
      throw new Error("This controller commissioning campaign is already recovered in Manual; no further rollback is needed.");
    }
  }

  private async execute(): Promise<CommissioningResult> {
    let status: CommissioningResult["status"] = "failed";
    let reason = "Controller commissioning did not finish.";
    let recording: LoopRecording | undefined;
    let parameters: ParameterStatus | undefined;
    let maximumError: number | undefined;
    try {
      await this.journal?.save("prepared");
      this.checkCancelled();
      const persistence = this.journal?.recordingPersistence();
      this.recording = await RuntimeRecordingSession.start(this.loop, persistence ? {
        persist: async owner => { this.checkCancelled(); await persistence.persist(owner); this.checkCancelled(); },
        clear: owner => persistence.clear(owner),
      } : undefined);
      this.checkCancelled();
      const beforeManual = await this.transfer.status();
      this.checkStartingPoint(beforeManual);
      const currentParameters = await this.parameters.status();
      if (currentParameters.program_revision !== this.review.observed.program_revision
        || currentParameters.binding_revision !== this.review.observed.binding_revision
        || currentParameters.active_revision !== this.review.observed.active_revision
        || !sameParameterValues(currentParameters.values, this.review.observed.values)
        || beforeManual.values[this.transfer.binding.mode.symbol] !== this.reviewedMode
        || Object.entries(this.previousReferences).some(([symbol, value]) => number(beforeManual, symbol) !== value)) {
        throw new Error("Reviewed parameters, mode or reference changed before commissioning; make a fresh review.");
      }
      if (beforeManual.values[this.transfer.binding.mode.symbol] !== this.transfer.binding.mode.manual_value) {
        await this.transfer.apply(this.profile.states(this.transfer.binding, beforeManual), true,
          transferTolerance(this.loop, this.task.transferTolerance), this.profile.tracking(this.loop, this.transfer.binding, false), beforeManual);
      }
      this.checkCancelled();
      await this.journal?.save("parameters");
      this.parameterApplicationAttempted = true;
      this.appliedParameterRevision = (await this.parameters.apply(this.review, false)).active_revision;
      this.checkCancelled();
      const beforeAuto = await this.transfer.status();
      this.checkStartingPoint(beforeAuto);
      if (Object.entries(this.review.values).some(([symbol, value]) => beforeAuto.values[symbol] !== value)) {
        throw new Error("The staged controller parameters changed before Auto entry.");
      }
      await this.transfer.apply(this.profile.states(this.transfer.binding, beforeAuto), false,
        autoEntryTolerance(this.loop, this.model, this.task, this.review.values), this.profile.tracking(this.loop, this.transfer.binding), beforeAuto);
      this.checkCancelled();
      const entered = await this.transfer.status();
      const target = this.profile.states(this.transfer.binding, entered);
      Object.assign(target, this.profile.targets?.(this.model, this.task) ?? { [this.transfer.binding.states[0].symbol]: this.target() });
      await this.transfer.apply(target, false, transferTargetAllowance(this.transfer.binding),
        this.profile.tracking(this.loop, this.transfer.binding, false), entered);
      // The reviewed duration is observed in the runtime's own time, which may run behind real time.
      if (!this.clock) throw new Error("The runtime clock is unavailable for observing the reviewed response.");
      await awaitRuntimeDuration(this.clock, this.task.durationSeconds * 1000 + this.loop.controller.taskPeriodMs * 2,
        { checkCancelled: () => this.checkCancelled(), boundMs: runtimeObservationBoundMs(this.task.durationSeconds) });
      recording = await this.recording.stop();
      maximumError = assessCommissioningRecording(recording, this.criteria());
      parameters = await this.parameters.status();
      if (parameters.program_revision !== this.review.observed.program_revision || !sameParameterValues(parameters.values, this.review.values)) {
        throw new Error("Controller parameters changed during commissioning.");
      }
      status = "passed";
      reason = `${this.profile.label} Auto commissioning passed: recorded target error ${formatEngineeringNumber(maximumError, 5)} ${this.loop.signals.processVariable.unit}; controller remains in Auto.`;
    } catch (error) {
      if (error instanceof RecordingStartUnconfirmed) { this.recording = error.session; recording = error.recording; }
      status = this.cancelled ? "cancelled" : "failed";
      reason = error instanceof Error ? error.message : String(error);
      try { await this.recover(); parameters = await this.parameters.status(); reason = withRecoveryOutcome(reason, recoveryConfirmedReason()); }
      catch (recovery) { this.recoveryConfirmed = false; reason = withRecoveryOutcome(reason, `Recovery unconfirmed: ${String(recovery)}`); }
    } finally {
      if (!recording && this.recording) {
        try { recording = await this.recording.stop(); }
        catch (error) { this.recoveryConfirmed = false; reason += ` Recording stop unconfirmed: ${String(error)}. Cancel retries the owned Stop.`; }
      }
    }
    await this.journal?.save(status === "passed" ? "completed" : this.recoveryConfirmed ? "recovered" : "transfer", { recordingResult: recording });
    return { status, reason, ...(recording ? { recording } : {}), transfers: this.transfer.outcomes,
      recoveryConfirmed: this.recoveryConfirmed, ...(maximumError === undefined ? {} : { maximumError }), ...(parameters ? { parameters } : {}),
      target: this.target(), modelRevision: this.model.revision, criteria: this.criteria(),
      channels: [this.loop.signals.setpoint!, this.loop.signals.processVariable, this.loop.signals.manipulatedVariable, ...(this.loop.signals.additional ?? [])]
        .map(signal => ({ id: signal.symbol, unit: signal.unit })) };
  }

  private async recover(): Promise<void> {
    if (!this.parameterApplicationAttempted && !this.transfer.hasOwnedOperation()) { this.recoveryConfirmed = true; return; }
    if (this.parameterOwner) await this.parameters.cancelOwner(this.parameterOwner);
    await this.transfer.cancel();
    const parameterState = await this.parameters.status();
    const restoreValues = this.recoveryValues ?? this.review.observed.values;
    const candidateValues = this.recoveryValues ? this.review.observed.values : this.review.values;
    const originalRevision = this.recoveryValues ? this.review.observed.previous_revision : this.review.observed.active_revision;
    const appliedRevision = this.appliedParameterRevision ?? (this.recoveryValues ? this.review.observed.active_revision : undefined);
    const unchanged = parameterState.active_revision === originalRevision && sameParameterValues(parameterState.values, restoreValues);
    const adopted = sameParameterValues(parameterState.values, candidateValues) && (appliedRevision !== undefined
      ? parameterState.active_revision === appliedRevision : parameterState.previous_revision === originalRevision);
    if (parameterState.status === "pending" || parameterState.program_revision !== this.review.observed.program_revision
      || parameterState.binding_revision !== this.review.observed.binding_revision || !unchanged && !adopted) {
      throw new Error("Parameter ownership changed; recovery will not overwrite another operation's values.");
    }
    const current = await this.transfer.status();
    if (current.program_revision !== this.review.observed.program_revision) throw new Error("Program identity changed; PID recovery cannot write to the new program.");
    const states = this.profile.states(this.transfer.binding, current);
    Object.assign(states, this.previousReferences);
    states[this.transfer.binding.states[1].symbol] = number(current, this.transfer.binding.output.symbol);
    if (current.values[this.transfer.binding.mode.symbol] !== this.transfer.binding.mode.manual_value
      || Object.entries(this.previousReferences).some(([symbol, value]) => number(current, symbol) !== value)) {
      await this.transfer.apply(states, true, transferTolerance(this.loop, this.task.transferTolerance),
        this.profile.tracking(this.loop, this.transfer.binding, false), current);
    }
    if (this.parameterApplicationAttempted) {
      const currentParameters = await this.parameters.status();
      if (currentParameters.status === "pending" || currentParameters.program_revision !== this.review.observed.program_revision ||
          currentParameters.binding_revision !== this.review.observed.binding_revision) throw new Error("Controller parameter recovery is unresolved or its identity changed.");
      if (!sameParameterValues(currentParameters.values, restoreValues)) {
        const rollback = await this.parameters.reviewRollback();
        if (!sameParameterValues(rollback.values, restoreValues)) throw new Error("The retained controller rollback no longer matches this campaign.");
        this.parameterOwner = { id: rollback.transaction_id!, program: rollback.observed.program_revision, binding: rollback.observed.binding_revision };
        await this.journal?.save("parameters", { parameterOperation: this.parameterOwner });
        await this.parameters.rollback(rollback, false);
      }
      this.parameterApplicationAttempted = false;
    }
    this.recoveryConfirmed = true;
  }

  private checkStartingPoint(observed: TransferStatus): void {
    assertCommissioningStartingPoint(this.loop, this.model, this.task, ...startingPointSymbols(this.loop, this.model), observed.values);
  }

  private referenceSymbols(): readonly string[] { return this.profile.references?.(this.loop) ?? [this.transfer.binding.states[0].symbol]; }
  private target(): number { return commissioningReferenceTargets(this.loop, this.model, this.task)[this.loop.signals.setpoint!.symbol]; }
  private checkCancelled(): void { if (this.cancelled) throw new Error("Controller commissioning cancelled."); }
  private criteria(): CommissioningCriteria { return { observed: commissioningObservation(this.loop, this.model, this.task, this.review.values), target: this.target(), durationSeconds: this.task.durationSeconds,
    settlingTimeSeconds: this.task.settlingTimeSeconds, maximumError: this.task.maximumError, taskPeriodMs: this.loop.controller.taskPeriodMs,
    ...(this.loop.controller.onOff ? { switching: { low: this.loop.controller.onOff.lowOutput, high: this.loop.controller.onOff.highOutput,
      minimumOnSeconds: this.review.values[this.loop.controller.onOff.minimumOnTimeSymbol],
      minimumOffSeconds: this.review.values[this.loop.controller.onOff.minimumOffTimeSymbol] } } : {}),
    processRange: { ...this.loop.signals.processVariable.range }, outputLimits: { minimum: this.transfer.binding.output.minimum, maximum: this.transfer.binding.output.maximum } }; }
}

/** The applied plant inputs and measured outputs whose validated operating point Apply starts from. */
function startingPointSymbols(loop: ControlLoopDefinition, model: GeneralModel): [string[], string[]] {
  const channels = connectedModelChannels(loop, model);
  return [channels.inputs.map(input => (input.applied ?? input.signal).symbol), channels.outputs.map(output => output.signal.symbol)];
}

function number(status: TransferStatus, symbol: string): number {
  const value = status.values[symbol];
  if (typeof value !== "number" || !Number.isFinite(value)) throw new Error(`The runtime did not return a finite ${symbol} value.`);
  return value;
}

/** The failed commissioning outcome once recovery is confirmed, for every controller this commissioning drives. */
/** Adds the recovery outcome to a failure reason as its own sentence. */
export function withRecoveryOutcome(reason: string, outcome: string): string {
  const failure = reason.trim();
  return `${/[.!?]$/.test(failure) ? failure : `${failure}.`} ${outcome}`;
}

export function recoveryConfirmedReason(): string {
  return "Recovery confirmed in Manual; previous controller parameters restored.";
}
