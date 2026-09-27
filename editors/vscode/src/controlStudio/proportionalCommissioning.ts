import { commissioningObservation } from "./commissioningObservation";
import { transferOutputBasis, transferTargetAllowance } from "./transferActuatorFeedback";
import type { ParameterOwner } from "./parameterApplication";
import { CommissioningJournal, type CommissioningCheckpointWriter } from "./commissioningCheckpoint";
import { RecordingStartUnconfirmed } from "./recordingStartFailure";
import type { RecordingSession } from "./recordingCoordinator";
import { matchesGainRecovery, type GainRecoveryContext } from "./gainRecoveryContext";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { GeneralModel } from "./generalModel";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import { proportionalParameter } from "./engineerVerificationPlan";
import { formatEngineeringNumber } from "./chartContract";
import { assessCommissioningRecording, type CommissioningCriteria } from "./commissioningAssessment";
import { sameParameterValues, type ParameterApplicationClient, type ParameterReview, type ParameterStatus } from "./parameterApplication";
import type { LoopRecording } from "./loopMonitor";
import { RuntimeRecordingSession } from "./runtimeRecording";
import { bindRuntimeConnection } from "./runtimeConnection";
import { RuntimeTransferClient, type TransferStatus } from "./runtimeTransfer";
import { proportionalTransferBinding, proportionalTrackingReference, proportionalTracking, roundedTransferValue, transferTolerance } from "./proportionalTransfer";

export interface CommissioningResult {
  status: "passed" | "failed" | "cancelled";
  reason: string; recording?: LoopRecording; transfers: readonly TransferStatus[];
  recoveryConfirmed: boolean; maximumError?: number; target: number;
  parameters?: ParameterStatus;
  modelRevision: string; channels: readonly { id: string; unit: string }[];
  criteria: CommissioningCriteria;
  gainRecovery?: GainRecoveryContext;
}

type RecoveryOrigin = Pick<ParameterStatus, "program_revision" | "binding_revision" | "active_revision" | "values">;

/** Coordinates reviewed commands and observations; the control law stays in ST. */
export class ProportionalCommissioning {
  private parameterOwner: ParameterOwner | undefined;
  private journal: CommissioningJournal | undefined;
  private reviewedMode: boolean | undefined;
  private cancelled = false;
  private running: Promise<CommissioningResult> | undefined;
  private finished: CommissioningResult | undefined;
  private parameterApplicationAttempted = false;
  private recording: RecordingSession | undefined;
  recoveryConfirmed = true;
  private constructor(private readonly loop: ControlLoopDefinition, private readonly model: GeneralModel,
    private readonly task: EngineerRequirements, private readonly parameters: ParameterApplicationClient,
    private readonly parameterReview: ParameterReview, private readonly transfer: RuntimeTransferClient,
    private readonly previousReference: number,
    private readonly historicalRecovery?: { origin: RecoveryOrigin; candidate: Readonly<Record<string, number>>; transfers: readonly TransferStatus[] }) {}

  static async review(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
    parameters: ParameterApplicationClient, parameterReview: ParameterReview): Promise<ProportionalCommissioning> {
    const connection = await bindRuntimeConnection(loop.runtimeId);
    if (connection.source !== "simulation") throw new Error("This commissioning adapter requires the local simulator.");
    const binding = proportionalTransferBinding(loop);
    const transfer = new RuntimeTransferClient(binding, parameterReview.observed.program_revision, connection.request);
    const observed = await transfer.status();
    if (!observed.supported_tracking?.includes("proportional")) throw new Error("The runtime needs task-boundary P tracking support before Auto commissioning.");
    if (observed.program_revision !== parameterReview.observed.program_revision) throw new Error("The program changed during commissioning review.");
    const result = new ProportionalCommissioning(loop, structuredClone(model), { ...task }, parameters, parameterReview,
      transfer, Number(observed.values[binding.states[0].symbol]));
    result.reviewedMode = observed.values[binding.mode.symbol] as boolean;
    result.checkStartingPoint(observed);
    result.trackingReference(observed, parameterReview.values[proportionalParameter(loop).symbol]);
    return result;
  }

  static assertRecoveryNeeded(loop: ControlLoopDefinition, report: CommissioningResult): void {
    const binding = proportionalTransferBinding(loop);
    const last = report.transfers[report.transfers.length - 1]?.transaction;
    if (report.recoveryConfirmed !== false && last?.confirmation === "output-image" && last.observed_values?.[binding.mode.symbol] === binding.mode.manual_value) {
      throw new Error("This commissioning campaign is already recovered in Manual; no further rollback is needed.");
    }
  }

  static async reviewRecovery(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
    parameters: ParameterApplicationClient, review: ParameterReview, report: CommissioningResult): Promise<ProportionalCommissioning> {
    ProportionalCommissioning.assertRecoveryNeeded(loop, report);
    const binding = proportionalTransferBinding(loop);
    const saved = report.parameters;
    const gainRecovery = report.gainRecovery;
    const first = report.transfers[0]?.transaction;
    if ((report.status !== "passed" && report.recoveryConfirmed !== false) || model.revision !== report.modelRevision || (!gainRecovery && (!saved?.previous_revision
      || !saved.previous_values)) || !first?.initial_values || first.confirmation !== "output-image") {
      throw new Error("The historical study lacks the original reference, gain set or compatible model needed for recovery.");
    }
    const current = review.observed;
    if (gainRecovery ? !matchesGainRecovery(current, gainRecovery)
      : !saved || current.program_revision !== saved.program_revision || current.binding_revision !== saved.binding_revision
        || current.active_revision !== saved.active_revision || current.previous_revision !== saved.previous_revision
        || !sameParameterValues(current.values, saved.values) || !sameParameterValues(review.values, saved.previous_values!)) {
      throw new Error("The running controller or retained gain history changed since this commissioning record.");
    }
    const connection = await bindRuntimeConnection(loop.runtimeId);
    if (connection.source !== "simulation") throw new Error("Historical recovery requires the reviewed local simulator.");
    const previousReference = first.initial_values[binding.states[0].symbol];
    if (typeof previousReference !== "number") throw new Error("The recorded original reference is unavailable.");
    roundedTransferValue(binding.states[0], previousReference);
    const transfer = new RuntimeTransferClient(binding, current.program_revision, connection.request);
    const observed = await transfer.status();
    if (observed.program_revision !== current.program_revision || first.program_revision !== current.program_revision
      || observed.binding_revision !== first.binding_revision || !observed.supported_tracking?.includes("proportional")) {
      throw new Error("The running program or transfer binding differs from the commissioning evidence.");
    }
    const result = new ProportionalCommissioning(loop, structuredClone(model), { ...task }, parameters, review, transfer, previousReference,
      { origin: { program_revision: current.program_revision, binding_revision: current.binding_revision,
        active_revision: current.previous_revision!, values: { ...review.values } }, candidate: { ...current.values }, transfers: structuredClone(report.transfers) });
    result.parameterApplicationAttempted = true;
    result.finished = { ...structuredClone(report), gainRecovery: result.recoveryContext() };
    return result;
  }

  rollbackDescription(): string {
    return `Return to Manual tracking the current output, restore reference ${formatEngineeringNumber(this.previousReference)} ${this.loop.signals.processVariable.unit}, then restore the previous gain set. Maximum tracking change: ${formatEngineeringNumber(transferTolerance(this.loop, this.task.transferTolerance), 8)} ${this.loop.signals.manipulatedVariable.unit}. Confirm this recovery on the currently reviewed runtime.`;
  }

  description(): string {
    return `Stage the verified gain in Manual, align the reference, enter Auto, then apply target ${formatEngineeringNumber(this.target())} ${this.loop.signals.processVariable.unit} and observe for ${this.task.durationSeconds} s. Maximum Auto-entry change: ${formatEngineeringNumber(transferTolerance(this.loop, this.task.transferTolerance), 8)} ${this.loop.signals.manipulatedVariable.unit}. Recovery tracks the observed output in Manual and restores the previous reference and gain.`;
  }

  setCheckpointWriter(writer: CommissioningCheckpointWriter): void {
    if (this.running) throw new Error("Install commissioning persistence before starting the campaign.");
    if (!this.parameterReview.transaction_id || this.parameterReview.observed.supports_request_id !== true) throw new Error("Reload the runtime with recoverable parameter operations before commissioning.");
    this.parameterOwner = { id: this.parameterReview.transaction_id, program: this.parameterReview.observed.program_revision, binding: this.parameterReview.observed.binding_revision };
    this.journal = new CommissioningJournal(this.loop, { model: this.model, task: this.task,
      origin: this.parameterReview.observed, candidate: this.parameterReview.values,
      parameterOperation: { id: this.parameterReview.transaction_id, program: this.parameterReview.observed.program_revision, binding: this.parameterReview.observed.binding_revision }, references: { [this.transfer.binding.states[0].symbol]: this.previousReference },
      binding: this.transfer.binding, recoveryTracking: proportionalTracking(this.loop, this.transfer.binding, false), tolerance: transferTolerance(this.loop, this.task.transferTolerance) }, writer);
    this.transfer.setOwnerPersistence(owner => this.journal!.save("transfer", { transfer: owner }));
  }

  run(): Promise<CommissioningResult> {
    if (this.historicalRecovery) throw new Error("This fresh review permits recovery only, not Auto commissioning.");
    if (this.running) throw new Error("The reviewed commissioning campaign has already started.");
    this.running = this.execute().then(result => { this.finished = result; return result; }); return this.running;
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
          const recording = await this.recording?.stop();
          this.finished = { ...this.finished, ...(recording ? { recording } : {}), recoveryConfirmed: true, parameters: await this.parameters.status(),
            transfers: this.retainedTransfers(), reason: "Commissioning cancelled. Recovery confirmed in Manual with the previous gain set." };
        } catch (error) {
          this.recoveryConfirmed = false;
          this.finished = { ...this.finished, recoveryConfirmed: false, reason: `Recovery unconfirmed: ${String(error)}` };
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

  async settleForClose(): Promise<CommissioningResult | undefined> {
    try { return await this.cancel(); }
    catch (error) {
      if (this.running) return await this.running;
      if (this.finished) return { ...this.finished, recoveryConfirmed: false,
        reason: `${this.finished.reason} Close recovery unconfirmed: ${String(error)}` };
      throw error;
    }
  }

  private recoveryContext(): GainRecoveryContext {
    const origin = this.historicalRecovery?.origin ?? this.parameterReview.observed;
    return { origin: { program_revision: origin.program_revision, binding_revision: origin.binding_revision,
      active_revision: origin.active_revision, values: { ...origin.values } },
      candidate: { ...(this.historicalRecovery?.candidate ?? this.parameterReview.values) } };
  }

  async rollback(): Promise<ParameterStatus> {
    try {
      await this.recover();
      const parameters = await this.parameters.status();
      if (this.finished) this.finished = { ...this.finished, parameters, transfers: this.retainedTransfers(),
        recoveryConfirmed: true, reason: "Recovery confirmed in Manual; previous gains restored." };
      if (this.recording) await this.recording.stop();
      await this.journal?.save("recovered");
      return parameters;
    } catch (error) {
      this.recoveryConfirmed = false;
      if (this.finished) this.finished = { ...this.finished, status: "failed", recoveryConfirmed: false,
        transfers: this.retainedTransfers(), reason: `Recovery unconfirmed: ${String(error)}` };
      throw error;
    }
  }

  private retainedTransfers(): TransferStatus[] {
    return [...(this.historicalRecovery?.transfers ?? []), ...this.transfer.outcomes];
  }

  private async execute(): Promise<CommissioningResult> {
    let status: CommissioningResult["status"] = "failed";
    let reason = "Commissioning did not finish.";
    let maximumError: number | undefined;
    let captured: LoopRecording | undefined;
    let parameters: ParameterStatus | undefined;
    try {
      await this.journal?.save("prepared");
      this.checkCancelled();
      const persistence = this.journal?.recordingPersistence();
      this.recording = await RuntimeRecordingSession.start(this.loop, persistence ? {
        persist: async owner => { this.checkCancelled(); await persistence.persist(owner); this.checkCancelled(); },
        clear: owner => persistence.clear(owner),
      } : undefined);
      this.checkCancelled();
      await this.journal?.save("parameters");
      const boundary = await this.transfer.status(), currentParameters = await this.parameters.status();
      if (currentParameters.program_revision !== this.parameterReview.observed.program_revision
        || currentParameters.binding_revision !== this.parameterReview.observed.binding_revision
        || currentParameters.active_revision !== this.parameterReview.observed.active_revision
        || !sameParameterValues(currentParameters.values, this.parameterReview.observed.values)
        || boundary.values[this.transfer.binding.mode.symbol] !== this.reviewedMode
        || boundary.values[this.transfer.binding.states[0].symbol] !== this.previousReference) throw new Error("Parameters, mode or reference changed since commissioning review.");
      if (boundary.values[this.transfer.binding.mode.symbol] !== this.transfer.binding.mode.manual_value) {
        await this.transfer.apply(this.stateValues(boundary, this.previousReference), true, transferTolerance(this.loop, this.task.transferTolerance),
          proportionalTracking(this.loop, this.transfer.binding, false), boundary);
      }
      this.checkCancelled();
      this.parameterApplicationAttempted = true;
      await this.parameters.apply(this.parameterReview, false);
      this.checkCancelled();
      const observed = await this.transfer.status();
      this.checkStartingPoint(observed);
      const gain = this.parameterReview.values[proportionalParameter(this.loop).symbol];
      if (Object.entries(this.parameterReview.values).some(([symbol, value]) => observed.values[symbol] !== value)) {
        throw new Error("The staged controller parameters changed before Auto entry.");
      }
      const reference = this.trackingReference(observed, gain);
      await this.transfer.apply(this.stateValues(observed, reference), false, transferTolerance(this.loop, this.task.transferTolerance), proportionalTracking(this.loop, this.transfer.binding, true));
      this.checkCancelled();
      const automatic = await this.transfer.status();
      await this.transfer.apply(this.stateValues(automatic, this.target()), false,
        transferTargetAllowance(this.transfer.binding), proportionalTracking(this.loop, this.transfer.binding, false));
      const until = Date.now() + this.task.durationSeconds * 1000 + this.loop.controller.taskPeriodMs * 2;
      while (Date.now() < until) { this.checkCancelled(); await new Promise(resolve => setTimeout(resolve, 25)); }
      captured = await this.recording.stop();
      maximumError = assessCommissioningRecording(captured, this.criteria());
      parameters = await this.parameters.status();
      if (parameters.program_revision !== this.parameterReview.observed.program_revision
        || Object.entries(this.parameterReview.values).some(([symbol, value]) => parameters!.values[symbol] !== value)) {
        throw new Error("Controller parameters changed during commissioning.");
      }
      status = "passed"; reason = `Auto commissioning passed: recorded target error ${formatEngineeringNumber(maximumError, 5)} ${this.loop.signals.processVariable.unit}; controller remains in Auto.`;
    } catch (error) {
      if (error instanceof RecordingStartUnconfirmed) { this.recording = error.session; captured = error.recording; }
      status = this.cancelled ? "cancelled" : "failed";
      reason = error instanceof Error ? error.message : String(error);
      try {
        await this.recover(); parameters = await this.parameters.status();
        reason += " Recovery confirmed in Manual; previous gains restored.";
      }
      catch (recovery) { this.recoveryConfirmed = false; reason += ` Recovery unconfirmed: ${String(recovery)}`; }
    } finally {
      if (!captured && this.recording) {
        try { captured = await this.recording.stop(); }
        catch (error) { this.recoveryConfirmed = false; reason += ` Recording stop unconfirmed: ${String(error)}. Cancel retries the owned Stop.`; }
      }
    }
    await this.journal?.save(status === "passed" ? "completed" : this.recoveryConfirmed ? "recovered" : "transfer", { recordingResult: captured });
    return { status, reason, ...(captured ? { recording: captured } : {}), transfers: this.retainedTransfers(),
      recoveryConfirmed: this.recoveryConfirmed, ...(maximumError === undefined ? {} : { maximumError }),
      ...(parameters ? { parameters } : {}), target: this.target(), modelRevision: this.model.revision,
      criteria: this.criteria(), gainRecovery: this.recoveryContext(),
      channels: [this.loop.signals.setpoint!, this.loop.signals.processVariable, this.loop.signals.manipulatedVariable]
        .map(signal => ({ id: signal.symbol, unit: signal.unit })) };
  }

  private async recover(): Promise<void> {
    if (this.parameterOwner) await this.parameters.cancelOwner(this.parameterOwner);
    await this.transfer.cancel();
    const original = this.historicalRecovery?.origin ?? this.parameterReview.observed;
    const candidate = this.historicalRecovery?.candidate ?? this.parameterReview.values;
    const before = this.parameterApplicationAttempted ? await this.parameters.status() : undefined;
    if (before && (before.program_revision !== original.program_revision || before.binding_revision !== original.binding_revision
      || before.status === "pending" || !(before.active_revision === original.active_revision && sameParameterValues(before.values, original.values)
        || before.previous_revision === original.active_revision && sameParameterValues(before.values, candidate)))) {
      throw new Error("The running gain transaction no longer matches this recovery review.");
    }
    const current = await this.transfer.status();
    const binding = this.transfer.binding;
    if (before && Object.entries(before.values).some(([symbol, value]) => current.values[symbol] !== value)) {
      throw new Error("The gain changed while reviewing the Manual transfer.");
    }
    if (current.program_revision !== original.program_revision) throw new Error("Program identity changed; recovery cannot write to the new program.");
    const values = this.stateValues(current, this.previousReference);
    if (current.values[binding.mode.symbol] !== binding.mode.manual_value || Object.entries(values).some(([symbol, value]) => current.values[symbol] !== value)) {
      await this.transfer.apply(values, true, transferTolerance(this.loop, this.task.transferTolerance), proportionalTracking(this.loop, binding, false), current);
    }
    if (this.parameterApplicationAttempted) {
      const status = await this.parameters.status();
      if (status.program_revision !== original.program_revision || status.binding_revision !== original.binding_revision
        || status.status === "pending") throw new Error("Parameter application is unresolved or its identity changed; inspect before recovery.");
      const alreadyRestored = status.active_revision === original.active_revision && sameParameterValues(status.values, original.values);
      if (!alreadyRestored) {
        const review = await this.parameters.reviewRollback();
        if (review.observed.program_revision !== original.program_revision || review.observed.binding_revision !== original.binding_revision
          || review.observed.previous_revision !== original.active_revision || !sameParameterValues(review.values, original.values)
          || !sameParameterValues(review.observed.values, candidate)) {
          throw new Error("The retained gain rollback no longer matches this campaign.");
        }
        this.parameterOwner = { id: review.transaction_id!, program: review.observed.program_revision, binding: review.observed.binding_revision };
        await this.journal?.save("parameters", { parameterOperation: this.parameterOwner });
        await this.parameters.rollback(review, false);
      }
      this.parameterApplicationAttempted = false;
    }
    this.recoveryConfirmed = true;
  }

  private stateValues(observed: TransferStatus, reference: number): Record<string, number> {
    const binding = this.transfer.binding;
    return { [binding.states[0].symbol]: roundedTransferValue(binding.states[0], reference),
      [binding.states[1].symbol]: roundedTransferValue(binding.states[1], Number(observed.values[transferOutputBasis(binding, binding.output).symbol])) };
  }
  private trackingReference(observed: TransferStatus, gain: number): number {
    return proportionalTrackingReference(this.transfer.binding, Number(observed.values[this.loop.signals.processVariable.symbol]),
      Number(observed.values[transferOutputBasis(this.transfer.binding, this.transfer.binding.output).symbol]), gain);
  }
  private checkStartingPoint(observed: TransferStatus): void {
    const coordinates = [[transferOutputBasis(this.transfer.binding, this.transfer.binding.output).symbol, this.model.operatingPoint?.inputs[0] ?? 0],
      [this.loop.signals.processVariable.symbol, this.model.operatingPoint?.outputs[0] ?? 0]] as const;
    if (coordinates.some(([symbol, value]) => Math.abs(Number(observed.values[symbol]) - value) > 1e-6 * Math.max(1, Math.abs(value)))) {
      throw new Error("The running process does not match the selected verified operating point; verify compatible starting conditions first.");
    }
  }
  private target(): number { return (this.model.operatingPoint?.outputs[0] ?? 0) + this.task.referenceChange; }
  private checkCancelled(): void { if (this.cancelled) throw new Error("Commissioning cancelled."); }
  private criteria(): CommissioningCriteria {
    return { observed: commissioningObservation(this.loop, this.model, this.task, this.parameterReview.values), target: this.target(), durationSeconds: this.task.durationSeconds,
      settlingTimeSeconds: this.task.settlingTimeSeconds, maximumError: this.task.maximumError,
      taskPeriodMs: this.loop.controller.taskPeriodMs, processRange: { ...this.loop.signals.processVariable.range },
      outputLimits: { minimum: this.transfer.binding.output.minimum, maximum: this.transfer.binding.output.maximum } };
  }

}
