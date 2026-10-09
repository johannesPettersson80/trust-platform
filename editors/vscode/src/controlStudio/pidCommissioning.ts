import { transferOutputBasis, transferTargetAllowance } from "./transferActuatorFeedback";
import type { ParameterOwner } from "./parameterApplication";
import { CommissioningJournal, type CommissioningCheckpointWriter } from "./commissioningCheckpoint";
import { commissioningObservation, commissioningTarget } from "./commissioningObservation";
import { connectedModelChannels } from "./loopChannels";
import type { CommissioningProfile } from "./commissioningProfile";
import { formatEngineeringNumber } from "./chartContract";
import type { CommissioningCriteria } from "./commissioningAssessment";
import type { CommissioningResult } from "./proportionalCommissioning";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import type { GeneralModel } from "./generalModel";
import type { ControlLoopDefinition } from "./loopDefinition";
import { bindRuntimeConnection } from "./runtimeConnection";
import { requireLiveWrite } from "./runtimeTarget";
import { RuntimeRecordingSession, type RecordingOwnerPersistence } from "./runtimeRecording";
import { RuntimeTransferClient, type TransferStatus } from "./runtimeTransfer";
import { runtimeClockNs } from "./runtimeDuration";
import { connectRuntimeParameters } from "./runtimeParameterConnection";
import type { ParameterApplicationClient, ParameterReview, ParameterStatus } from "./parameterApplication";
import { sameParameterValues } from "./parameterApplication";
import { parallelPidTracking, pidTransferBinding, pidTransferStates } from "./pidTransfer";
import { transferTolerance } from "./proportionalTransfer";
import { anchorIntegratingOutputs, assertCommissioningStartingPoint, assertReferenceHeld, autoEntryError, autoEntryTolerance, awaitCommissioningStartingPoint, comparedPlantInputs, withReferenceHoldGuard, type StartingPointWait } from "./commissioningStartingPoint";
import type { LoopRecording } from "./loopMonitor";
import { RecordingStartUnconfirmed } from "./recordingStartFailure";
import { MainWorkflowError, type MainWorkflowFailure } from "./mainWorkflowFailure";
import { ObservationDetached, ObservationRecord, ObservationRequirementFailed, PermittedRangeLeft, assessObservation, observeTargetChange } from "./applyObservation";
import { foundModeOf, reenterFoundMode, reviewedModeOf, type FoundMode } from "./foundModeReturn";

const pidProfile: CommissioningProfile = { label: "PID", trackingKind: "parallel-pid-v1",
  binding: pidTransferBinding, states: pidTransferStates, tracking: parallelPidTracking };

/** Shared reviewed stateful transfer lifecycle; numerical/controller laws stay in their adapters. */
export class PidCommissioning {
  private previousReferences: Record<string, number> = {};
  private reviewedMode: boolean | undefined;
  /** The measured output's noise as single executions see it (A6.4): it sets the allowance of each Auto entry. */
  private processNoise: number | undefined;
  private appliedParameterRevision: string | undefined;
  private recoveryValues: Readonly<Record<string, number>> | undefined;
  private parameterOwner: ParameterOwner | undefined;
  private journal: CommissioningJournal | undefined;
  private observation: ObservationRecord | undefined;
  private cancelled = false;
  private running: Promise<CommissioningResult> | undefined;
  private finished: CommissioningResult | undefined;
  private recording: RuntimeRecordingSession | undefined;
  /** Set while the runtime records the response of the applied settings (B6.3): closing the panel does not abort it. */
  private observing = false;
  /** Set when the panel closes during the observation: the runtime keeps recording it, and reopening reattaches it (A8.5, A10.4). */
  private detachedForClose = false;
  /** The page's running status for the observation: the progress of the starting-point wait, kept from the review (B6.3, A3.6). */
  private observationProgress: ((text: string) => void | Promise<void>) | undefined;
  private parameterApplicationAttempted = false;
  /** A failed requirement keeps the reviewed settings applied until the engineer decides on Restore previous settings (B6.3). */
  private awaitingRestore = false;
  recoveryConfirmed = true;

  private constructor(private readonly loop: ControlLoopDefinition, private readonly model: GeneralModel,
    private readonly task: EngineerRequirements, private readonly parameters: ParameterApplicationClient,
    private readonly review: ParameterReview, private readonly transfer: RuntimeTransferClient,
    private readonly previousReference: number, private readonly profile: CommissioningProfile) {}

  static async review(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
    parameters: ParameterApplicationClient, review: ParameterReview, profile: CommissioningProfile = pidProfile,
    wait?: StartingPointWait, processNoise?: number): Promise<PidCommissioning> {
    const connection = await bindRuntimeConnection(loop.runtimeId);
    requireLiveWrite(connection, "Auto commissioning");
    const binding = withReferenceHoldGuard(loop, profile.binding(loop));
    const transfer = new RuntimeTransferClient(binding, review.observed.program_revision, connection.request);
    const first = await transfer.status();
    if (![profile.trackingKind, ...(profile.requiredTracking ?? [])].every(kind => first.supported_tracking?.includes(kind))) throw new Error(`The runtime needs ${profile.trackingKind} state tracking before Auto commissioning.`);
    assertReferenceHeld(loop, first.values);
    const [inputs, outputs] = startingPointSymbols(loop, model);
    const compared = comparedPlantInputs(inputs, foundModeOf(first.values[binding.mode.symbol] as boolean, binding.mode.manual_value));
    model = anchorIntegratingOutputs(model, first.values, outputs);
    // B6.2: a plant input held in Manual must lie within the candidate's Auto-entry allowance of the validated point (A6.4).
    const allowance = autoEntryTolerance(loop, model, task, review.values, processNoise, autoEntryError(loop, first.values));
    const observed = wait ? await awaitCommissioningStartingPoint(loop, model, task, compared, outputs, () => transfer.status(), wait, allowance) : first;
    if (observed.program_revision !== review.observed.program_revision) throw new Error("The program changed during PID commissioning review.");
    const result = new PidCommissioning(loop, structuredClone(model), { ...task }, parameters, review, transfer,
      number(observed, binding.states[0].symbol), profile);
    result.previousReferences = Object.fromEntries(result.referenceSymbols().map(symbol => [symbol, number(observed, symbol)]));
    result.reviewedMode = observed.values[binding.mode.symbol] as boolean;
    result.processNoise = processNoise;
    result.observationProgress = wait?.progress.bind(wait);
    result.checkStartingPoint(observed);
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
    requireLiveWrite(connection, "controller recovery");
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
    // Restore returns the loop to the mode Apply found it in (A6.2, B6.4).
    result.reviewedMode = reviewedModeOf(report.foundMode, binding.mode.manual_value);
    return result;
  }

  description(): string {
    return `Apply the reviewed ${this.profile.label} settings and start Auto at ${formatEngineeringNumber(this.target())} ${this.loop.signals.processVariable.unit}. Monitor for ${this.task.durationSeconds} s.`;
  }

  rollbackDescription(): string {
    return `Return to Manual at the current applied output, restore reference ${formatEngineeringNumber(this.previousReference)} ${this.loop.signals.processVariable.unit}, then restore the previous ${this.profile.label} record${this.foundInAuto() ? " and return to Auto" : ""}. Confirm recovery on the reviewed runtime.`;
  }

  setCheckpointWriter(writer: CommissioningCheckpointWriter): void {
    if (this.running) throw new Error("Install commissioning persistence before starting the campaign.");
    if (!this.review.transaction_id || this.review.observed.supports_request_id !== true) throw new Error("Reload the runtime with recoverable parameter operations before commissioning.");
    this.parameterOwner = { id: this.review.transaction_id, program: this.review.observed.program_revision, binding: this.review.observed.binding_revision };
    this.observation = new ObservationRecord(writer);
    this.journal = new CommissioningJournal(this.loop, { model: this.model, task: this.task,
      origin: this.review.observed, candidate: this.review.values,
      parameterOperation: { id: this.review.transaction_id, program: this.review.observed.program_revision, binding: this.review.observed.binding_revision }, references: this.previousReferences,
      binding: this.transfer.binding, recoveryTracking: this.profile.tracking(this.loop, this.transfer.binding, false), tolerance: transferTolerance(this.loop, this.task.transferTolerance),
      foundMode: this.foundMode(), ...(this.processNoise !== undefined ? { processNoise: this.processNoise } : {}) }, this.observation.checkpointWriter());
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
            transfers: this.transfer.outcomes, recoveryConfirmed: true, reason: `Controller recovery and recording stop confirmed in ${this.modeName()}.` };
        } catch (error) {
          this.recoveryConfirmed = false;
          this.finished = { ...this.finished, recoveryConfirmed: false, reason: `Controller recovery remains unresolved: ${String(error)}` };
        }
      }
      if (this.finished.recoveryConfirmed && !this.awaitingRestore) await this.journal?.save("recovered", { recordingResult: this.finished.recording });
      return this.finished;
    }
    if (this.running) return this.running;
    return undefined;
  }

  /**
   * Closing the panel during the observation detaches it at once instead of waiting for it: the runtime keeps recording the response
   * and the journal keeps the observation, which reopening reattaches (A8.5, A10.4). Returns whether an observation was detached.
   */
  detachForClose(): boolean {
    if (!this.observing) return false;
    this.detachedForClose = true;
    void this.running?.catch(() => undefined);
    return true;
  }

  recordedOutcome(): CommissioningResult | undefined { return this.finished; }
  isCampaignRunning(): boolean { return this.running !== undefined && this.finished === undefined; }

  /** B6.3, A10.4: an Apply observation the runtime is recording survives closing the panel; its outcome is awaited, and reopening resumes it. */
  async settleForClose(): Promise<CommissioningResult | undefined> {
    if (this.observing) return this.running;
    return this.cancel();
  }

  /** Releases the runtime's buffer of the campaign's recording. Call it only after the samples are stored (runtimeRecording.ts). */
  async releaseRecording(): Promise<void> {
    try { await this.recording?.release(); }
    catch { /* A buffer that stays pending is swept by the next recording start (runtimeRecording.ts). */ }
  }

  async rollback(): Promise<ParameterStatus> {
    try {
      await this.recover();
      const parameters = await this.parameters.status();
      if (this.finished) this.finished = { ...this.finished, parameters, transfers: this.transfer.outcomes,
        recoveryConfirmed: true, failure: undefined, reason: `Controller recovery confirmed in ${this.modeName()}; previous parameters restored.` };
      if (this.recording) await this.recording.stop();
      await this.journal?.save("recovered");
      return parameters;
    } catch (error) {
      // A failed Restore leaves the recovery unresolved: the session keeps its interlock and the transfer evidence until a retry
      // succeeds (A6.3).
      this.recoveryConfirmed = false;
      if (this.finished) this.finished = { ...this.finished, status: "failed", recoveryConfirmed: false,
        transfers: this.transfer.outcomes, reason: `Recovery unconfirmed: ${String(error)}` };
      throw error;
    }
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
    let captured: LoopRecording | undefined;
    let parameters: ParameterStatus | undefined;
    let maximumError: number | undefined;
    let detached = false;
    let failure: MainWorkflowFailure | undefined;
    try {
      await this.journal?.save("prepared");
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
        autoEntryTolerance(this.loop, this.model, this.task, this.review.values, this.processNoise, this.entryError(beforeAuto)),
        this.profile.tracking(this.loop, this.transfer.binding), beforeAuto);
      this.checkCancelled();
      const entered = await this.transfer.status();
      const target = this.profile.states(this.transfer.binding, entered);
      Object.assign(target, this.profile.targets?.(this.model, this.task) ?? { [this.transfer.binding.states[0].symbol]: this.target() });
      // B6.3: after the Auto entry, the validated target change runs while the runtime records the response for the test duration.
      captured = await observeTargetChange(this.loop, {
        durationMs: this.task.durationSeconds * 1000,
        startedAtNs: runtimeClockNs(entered),
        persistence: this.recordingPersistence(),
        journal: this.observation,
        progress: text => this.observationProgress?.(text),
        checkCancelled: () => { if (this.detachedForClose) throw new ObservationDetached(); this.checkCancelled(); },
        onStarted: session => { this.recording = session; this.observing = true; },
        responseStart: sample => sample.controllerMode === "Auto" && sample.sp === this.criteria().target,
        transfers: this.transfer.outcomes,
        expectedSettlingSeconds: this.loop.controller.onOff ? undefined : this.criteria().settlingTimeSeconds,
        applyTarget: () => this.transfer.apply(target, false, transferTargetAllowance(this.transfer.binding),
          this.profile.tracking(this.loop, this.transfer.binding, false), entered),
      });
      maximumError = assessObservation(captured, this.criteria());
      parameters = await this.parameters.status();
      if (parameters.program_revision !== this.review.observed.program_revision || !sameParameterValues(parameters.values, this.review.values)) {
        throw new Error("Controller parameters changed during commissioning.");
      }
      status = "passed";
      reason = `${this.profile.label} Auto commissioning passed: recorded target error ${formatEngineeringNumber(maximumError, 5)} ${this.loop.signals.processVariable.unit}; controller remains in Auto.`;
    } catch (error) {
      // A detached observation is left whole to the runtime and its journal: nothing is restored, stopped or saved here (A8.5).
      if (error instanceof ObservationDetached) { detached = true; throw error; }
      if (error instanceof RecordingStartUnconfirmed) { if (error.session instanceof RuntimeRecordingSession) this.recording = error.session; captured = error.recording; }
      status = this.cancelled ? "cancelled" : "failed";
      reason = error instanceof Error ? error.message : String(error);
      failure = error instanceof MainWorkflowError ? error.failure : undefined;
      if (error instanceof ObservationRequirementFailed) {
        // B6.3: the reviewed settings stay applied; the failed requirement is reported and Restore previous settings is offered.
        this.awaitingRestore = true;
        try { parameters = await this.parameters.status(); }
        catch (statusError) { this.recoveryConfirmed = false; reason = withRecoveryOutcome(reason, `The applied settings could not be read: ${String(statusError)}`); }
      } else {
        try {
          await this.recover();
          parameters = await this.parameters.status();
          reason = error instanceof PermittedRangeLeft ? `${error.statement}; the previous settings were restored.` : withRecoveryOutcome(reason, this.recoveredText());
        } catch (recovery) {
          this.recoveryConfirmed = false;
          reason = withRecoveryOutcome(error instanceof PermittedRangeLeft ? error.statement : reason, `Recovery unconfirmed: ${String(recovery)}`);
        }
      }
    } finally {
      this.observing = false;
      if (!detached && !captured && this.recording) {
        try { captured = await this.recording.stop(); }
        catch (error) { this.recoveryConfirmed = false; reason += ` Recording stop unconfirmed: ${String(error)}. Cancel retries the owned Stop.`; }
      }
    }
    const stage = status === "passed" || this.awaitingRestore ? "completed" : this.recoveryConfirmed ? "recovered" : "transfer";
    await this.journal?.save(stage, { recordingResult: captured });
    return { status, reason, ...(captured ? { recording: captured } : {}), ...(this.foundMode() ? { foundMode: this.foundMode() } : {}), transfers: this.transfer.outcomes,
      recoveryConfirmed: this.recoveryConfirmed, ...(maximumError === undefined ? {} : { maximumError }), ...(parameters ? { parameters } : {}),
      ...(failure ? { failure } : {}), target: this.target(), modelRevision: this.model.revision, criteria: this.criteria(),
      channels: [this.loop.signals.setpoint!, this.loop.signals.processVariable, this.loop.signals.manipulatedVariable, ...(this.loop.signals.additional ?? [])]
        .map(signal => ({ id: signal.symbol, unit: signal.unit })) };
  }

  private async recover(): Promise<void> {
    if (!this.parameterApplicationAttempted && !this.transfer.hasOwnedOperation()) { this.awaitingRestore = false; this.recoveryConfirmed = true; return; }
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
    // A6.2: once the previous settings are back, the loop returns to the mode the operation found it in.
    await this.returnToFoundMode();
    this.awaitingRestore = false;
    this.recoveryConfirmed = true;
  }

  /** A6.2: a recovery that found the loop in Auto re-enters Auto with the restored settings and the previous reference. */
  private async returnToFoundMode(): Promise<void> {
    const restored = this.recoveryValues ?? this.review.observed.values;
    await reenterFoundMode(this.transfer, this.foundMode(), current => ({ values: this.profile.states(this.transfer.binding, current),
      tolerance: autoEntryTolerance(this.loop, this.model, this.task, restored, this.processNoise, this.entryError(current)),
      tracking: this.profile.tracking(this.loop, this.transfer.binding) }));
  }

  /** The error the controller enters Auto with: its setpoint less its measurement (A6.4). */
  private entryError(status: TransferStatus): number | undefined {
    return autoEntryError(this.loop, status.values);
  }

  /** The mode this session reviewed the loop in (A6.2). A historical recovery reviewed later knows no mode and keeps Manual. */
  private foundMode(): FoundMode | undefined {
    return foundModeOf(this.reviewedMode, this.transfer.binding.mode.manual_value);
  }

  private foundInAuto(): boolean {
    return this.foundMode() === "auto";
  }

  private modeName(): string {
    return this.foundInAuto() ? "Auto" : "Manual";
  }

  private recoveredText(): string {
    return recoveryConfirmedReason(this.foundInAuto() ? "auto" : "manual");
  }

  /** The commissioning journal's recording persistence, with cancellation checked before and after each durable write. */
  private recordingPersistence(): RecordingOwnerPersistence | undefined {
    const persistence = this.journal?.recordingPersistence();
    return persistence && {
      persist: async owner => { this.checkCancelled(); await persistence.persist(owner); this.checkCancelled(); },
      clear: owner => persistence.clear(owner),
    };
  }

  private referenceSymbols(): readonly string[] { return this.profile.references?.(this.loop) ?? [this.transfer.binding.states[0].symbol]; }
  /** The reviewed target rounded to its IEC type, as the runtime holds it, so the transfer, the report and the assessment agree. */
  private target(): number { return commissioningTarget(this.loop, this.model, this.task, this.transfer.binding); }
  private checkCancelled(): void { if (this.cancelled) throw new Error("Controller commissioning cancelled."); }
  private checkStartingPoint(observed: TransferStatus): void {
    const [inputs, outputs] = startingPointSymbols(this.loop, this.model);
    assertCommissioningStartingPoint(this.loop, this.model, this.task, comparedPlantInputs(inputs, this.foundMode()), outputs, observed.values,
      autoEntryTolerance(this.loop, this.model, this.task, this.review.values, this.processNoise, this.entryError(observed)));
  }
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

/** The recovery outcome of a PID campaign: the mode the loop is confirmed in (A6.2). */
export function recoveryConfirmedReason(mode: "auto" | "manual" = "manual"): string {
  return mode === "auto"
    ? "Recovery confirmed in Auto; previous controller parameters restored."
    : "Recovery confirmed in Manual; previous controller parameters restored.";
}
