import { isDeepStrictEqual } from "util";
import type { ControlLoopDefinition } from "./loopDefinition";
import { controllerAdapter } from "./controllerAdapters";
import { bindRuntimeConnection } from "./runtimeConnection";
import { RuntimeTransferClient, type TransferStatus } from "./runtimeTransfer";
import { autoEntryError, autoEntryTolerance } from "./commissioningStartingPoint";
import type { CommissioningProfile } from "./commissioningProfile";
import { reenterFoundMode, type AutoReentry } from "./foundModeReturn";
import { parallelPidTracking } from "./pidTransfer";
import { proportionalTracking, proportionalTrackingReference, roundedTransferValue, type ProportionalTracking } from "./proportionalTransfer";
import { transferOutputBasis } from "./transferActuatorFeedback";
import { RuntimeRecordingSession } from "./runtimeRecording";
import { stopRecovered } from "./applyObservation";
import { sameParameterValues, type ParameterApplicationClient, type ParameterReview, type ParameterStatus } from "./parameterApplication";
import type { CommissioningCheckpoint, CommissioningCheckpointWriter } from "./commissioningCheckpoint";
import type { CommissioningResult } from "./proportionalCommissioning";

/**
 * A checkpoint grants only a fresh reviewed recovery, never a replay: it restores the previous settings in Manual, and then returns
 * a loop found in Auto to Auto with them (A6.2, B6.4).
 */
export class InterruptedCommissioningRecovery {
  recoveryConfirmed = false;
  private consumed = false;
  /** Whether the last rollback returned the loop to Auto, once its previous settings were back (A6.2). */
  private returnedToAuto = false;
  private constructor(private readonly loop: ControlLoopDefinition, private checkpoint: CommissioningCheckpoint,
    private readonly parameters: ParameterApplicationClient, private readonly review: ParameterReview,
    private readonly transfer: RuntimeTransferClient, private readonly writer: CommissioningCheckpointWriter) {}

  static async review(loop: ControlLoopDefinition, checkpoint: CommissioningCheckpoint, parameters: ParameterApplicationClient,
    writer: CommissioningCheckpointWriter): Promise<InterruptedCommissioningRecovery> {
    const adapter = controllerAdapter(loop);
    if (checkpoint.loopRevision !== loop.revision || !isDeepStrictEqual(adapter.transferBinding(), checkpoint.binding)
      || !isDeepStrictEqual(adapter.recoveryTracking(checkpoint.model), checkpoint.recoveryTracking)) throw new Error("The commissioning checkpoint does not match the current controller binding.");
    const observedParameters = await parameters.status();
    assertParameterOwner(checkpoint, observedParameters, true);
    const review: ParameterReview = { observed: observedParameters, values: { ...checkpoint.origin.values } };
    const connection = await bindRuntimeConnection(loop.runtimeId);
    if (connection.source !== "simulation") throw new Error("Reconnect the original simulator before recovering commissioning.");
    const transfer = new RuntimeTransferClient(checkpoint.binding, checkpoint.origin.program_revision, connection.request);
    const observed = await transfer.status();
    if (observed.program_revision !== checkpoint.origin.program_revision
      || checkpoint.transfer && observed.binding_revision !== checkpoint.transfer.binding
      || observed.status === "pending" && observed.transaction?.id !== checkpoint.transfer?.id) throw new Error("Another program or transfer owns the current runtime; recovery cannot overwrite it.");
    if (checkpoint.transfer) {
      transfer.restoreOwner(checkpoint.transfer);
    }
    const recovery = new InterruptedCommissioningRecovery(loop, structuredClone(checkpoint), parameters, review, transfer, writer);
    transfer.setOwnerPersistence(async owner => {
      recovery.checkpoint = { ...recovery.checkpoint, stage: "transfer", transfer: owner };
      await writer(recovery.checkpoint);
    });
    return recovery;
  }
  parameterReview(): ParameterReview { return this.review; }
  description(): string { return this.rollbackDescription(); }
  rollbackDescription(): string {
    const base = this.checkpoint.stage === "completed"
      ? "Return to Manual at the current output and restore the previous settings."
      : "Recover the interrupted operation in Manual and restore the previous settings.";
    return `${base}${this.modeClause()}`;
  }
  /** What the review says about the mode the restore leaves the loop in (A6.2). */
  private modeClause(): string {
    if (this.checkpoint.foundMode === undefined) return " The mode the loop was found in is unknown, so it stays in Manual.";
    if (this.checkpoint.foundMode === "manual") return "";
    return this.reenterable() ? " Then the loop returns to Auto, the mode it was found in."
      : " Then the loop stays in Manual: its Auto entry needs the controller initialization, which this recovery does not hold.";
  }
  /** The mode the restore left the loop in, as the reason of the restore states it. Read it after rollback() (A6.2). */
  modeStatement(): string {
    if (this.returnedToAuto) return "the loop returned to Auto";
    if (this.checkpoint.foundMode === undefined) return "the loop stays in Manual, because the mode it was found in is unknown";
    if (this.checkpoint.foundMode === "auto" && !this.reenterable()) {
      return "the loop stays in Manual, because its Auto entry needs the controller initialization, which this recovery does not hold";
    }
    return "the loop stays in Manual";
  }
  setCheckpointWriter(_writer: CommissioningCheckpointWriter): void { /* Recovery already owns the durable writer supplied by its fresh review. */ }
  run(): Promise<CommissioningResult> { return Promise.reject(new Error("An interrupted checkpoint permits reviewed recovery only.")); }
  isCampaignRunning(): boolean { return false; }
  recordedOutcome(): CommissioningResult | undefined { return undefined; }
  async settleForClose(): Promise<CommissioningResult | undefined> { await this.transfer.cancel(); return undefined; }
  /** The recovery released its recording buffer in rollback(), once the checkpoint held the samples (runtimeRecording.ts). */
  async releaseRecording(): Promise<void> { /* Nothing is held here: rollback() released the buffer after the checkpoint was stored. */ }
  async cancel(): Promise<CommissioningResult | undefined> { await this.transfer.cancel(); return undefined; }

  async rollback(): Promise<ParameterStatus> {
    if (this.consumed) throw new Error("Review recovery again before another rollback attempt.");
    await this.parameters.cancelOwner(this.checkpoint.parameterOperation);
    await this.transfer.cancel();
    let parameters = await this.parameters.status();
    assertParameterOwner(this.checkpoint, parameters);
    const current = await this.transfer.status();
    if (current.program_revision !== this.checkpoint.origin.program_revision || current.status === "pending") throw new Error("Controller transfer ownership remains unresolved.");
    const states = this.recoveryStates(current);
    if (current.values[this.checkpoint.binding.mode.symbol] !== this.checkpoint.binding.mode.manual_value
      || Object.entries(this.checkpoint.references).some(([symbol, value]) => current.values[symbol] !== value)) {
      await this.transfer.apply(states, true, this.checkpoint.tolerance, this.checkpoint.recoveryTracking, current);
    }
    parameters = await this.parameters.status();
    assertParameterOwner(this.checkpoint, parameters);
    if (!sameParameterValues(parameters.values, this.checkpoint.origin.values)) {
      const rollback = await this.parameters.reviewRollback();
      if (!sameParameterValues(rollback.values, this.checkpoint.origin.values)) throw new Error("The original parameter record is no longer the runtime's rollback record.");
      this.checkpoint = { ...this.checkpoint, stage: "parameters", parameterOperation: { id: rollback.transaction_id!,
        program: rollback.observed.program_revision, binding: rollback.observed.binding_revision } };
      await this.writer(this.checkpoint);
      parameters = await this.parameters.rollback(rollback, false);
    }
    if (this.checkpoint.recording && !this.checkpoint.recordingResult) {
      const session = await RuntimeRecordingSession.recover(this.loop, this.checkpoint.recording);
      // A recording the runtime no longer holds is stored as incomplete evidence, and the settings are restored all the same (A8.6).
      const { recording: recordingResult, held } = await stopRecovered(session);
      this.checkpoint = { ...this.checkpoint, recordingResult };
      await this.writer(this.checkpoint);
      // The samples are stored in the checkpoint now, so the runtime's buffer can be released (runtimeRecording.ts).
      if (held) await session.release();
    }
    // A6.2: the loop returns to Auto only now, once the previous settings and the recording are confirmed back.
    this.returnedToAuto = await reenterFoundMode(this.transfer, this.checkpoint.foundMode, observed => this.autoReentry(observed));
    this.checkpoint = { ...this.checkpoint, stage: "recovered" };
    await this.writer(this.checkpoint);
    // Consume only after every owned operation, recording stop, and durable
    // checkpoint write has completed. A failed recovery must remain retryable
    // from this reviewed instance without replaying any controller write.
    this.consumed = true;
    this.recoveryConfirmed = true;
    return parameters;
  }

  /** The states a recovery writes: each declared state as observed, with the references the checkpoint keeps. */
  private recoveryStates(observed: TransferStatus): Record<string, number> {
    const states = Object.fromEntries(this.checkpoint.binding.states.map(member => {
      const value = observed.values[member.symbol];
      if (typeof value !== "number" || !Number.isFinite(value)) throw new Error(`Recovery state ${member.symbol} is unavailable.`);
      return [member.symbol, value];
    }));
    Object.assign(states, this.checkpoint.references);
    return states;
  }

  /** Whether the checkpoint's controller has an Auto entry: the PID and proportional laws, and a declared controller's (A6.2). */
  private reenterable(): boolean {
    const kind = this.checkpoint.recoveryTracking.kind;
    return kind === "parallel-pid-v1" || kind === "proportional" || (kind === "declared-state-v1" && !!this.declaredProfile());
  }

  /**
   * The Auto entry of the checkpoint's controller, built from the states observed once the previous settings are back (A6.2). A
   * declared controller, such as Control.PID, enters as Apply's commissioning enters it: through its declared initialization, with
   * the restored settings.
   */
  private autoReentry(observed: TransferStatus): AutoReentry | undefined {
    const { binding, recoveryTracking: tracking } = this.checkpoint;
    if (tracking.kind === "proportional") {
      return { values: this.proportionalEntryValues(observed, tracking), tolerance: this.entryTolerance(observed),
        tracking: proportionalTracking(this.loop, binding, true) };
    }
    if (tracking.kind === "parallel-pid-v1") {
      return { values: this.recoveryStates(observed), tolerance: this.entryTolerance(observed), tracking: parallelPidTracking(this.loop, binding, true) };
    }
    const profile = tracking.kind === "declared-state-v1" ? this.declaredProfile() : undefined;
    return profile && { values: profile.states(binding, observed), tolerance: this.entryTolerance(observed),
      tracking: profile.tracking(this.loop, binding, true) };
  }

  /** The declared controller's commissioning profile with the restored settings, when the loop's adapter declares one. */
  private declaredProfile(): CommissioningProfile | undefined {
    return controllerAdapter(this.loop).declaredProfile?.(this.checkpoint.model, this.checkpoint.origin.values);
  }

  /** The proportional entry aligns the reference with the current output at the restored gain, as ProportionalCommissioning does. */
  private proportionalEntryValues(observed: TransferStatus, tracking: ProportionalTracking): Record<string, number> {
    const { binding } = this.checkpoint;
    const output = Number(observed.values[transferOutputBasis(binding, binding.output).symbol]);
    const reference = proportionalTrackingReference(binding, Number(observed.values[tracking.process_variable]), output,
      this.checkpoint.origin.values[tracking.gain_symbol]);
    return { [binding.states[0].symbol]: roundedTransferValue(binding.states[0], reference),
      [binding.states[1].symbol]: roundedTransferValue(binding.states[1], output) };
  }

  /**
   * The Auto entry's allowed output change (A6.3, A6.4): the allowance Apply's commissioning uses, for the restored settings, the
   * noise Apply measured and the error the loop enters with.
   */
  private entryTolerance(observed: TransferStatus): number {
    return autoEntryTolerance(this.loop, this.checkpoint.model, this.checkpoint.task, this.checkpoint.origin.values,
      this.checkpoint.processNoise, autoEntryError(this.loop, observed.values));
  }
}
function assertParameterOwner(checkpoint: CommissioningCheckpoint, current: ParameterStatus, allowOwnedPending = false): void {
  const unchanged = current.active_revision === checkpoint.origin.active_revision && sameParameterValues(current.values, checkpoint.origin.values);
  const adopted = current.previous_revision === checkpoint.origin.active_revision && !!current.previous_values
    && sameParameterValues(current.previous_values, checkpoint.origin.values) && sameParameterValues(current.values, checkpoint.candidate);
  if (current.status === "pending" && (!allowOwnedPending || current.transaction_id !== checkpoint.parameterOperation.id) || current.program_revision !== checkpoint.origin.program_revision
    || current.binding_revision !== checkpoint.origin.binding_revision || !unchanged && !adopted) throw new Error("Parameter ownership changed or remains pending; interrupted recovery cannot overwrite another operation.");
}
