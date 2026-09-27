import { isDeepStrictEqual } from "util";
import type { ControlLoopDefinition } from "./loopDefinition";
import { controllerAdapter } from "./controllerAdapters";
import { bindRuntimeConnection } from "./runtimeConnection";
import { RuntimeTransferClient } from "./runtimeTransfer";
import { RuntimeRecordingSession } from "./runtimeRecording";
import { sameParameterValues, type ParameterApplicationClient, type ParameterReview, type ParameterStatus } from "./parameterApplication";
import type { CommissioningCheckpoint, CommissioningCheckpointWriter } from "./commissioningCheckpoint";
import type { CommissioningResult } from "./proportionalCommissioning";

/** A reopened checkpoint grants only a fresh reviewed recovery, never replay or Auto entry. */
export class InterruptedCommissioningRecovery {
  recoveryConfirmed = false;
  private consumed = false;
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
    return this.checkpoint.stage === "completed"
      ? "Return to Manual at the current output and restore the previous settings."
      : "Recover the interrupted operation in Manual and restore the previous settings.";
  }
  setCheckpointWriter(_writer: CommissioningCheckpointWriter): void { /* Recovery already owns the durable writer supplied by its fresh review. */ }
  run(): Promise<CommissioningResult> { return Promise.reject(new Error("An interrupted checkpoint permits reviewed recovery only.")); }
  isCampaignRunning(): boolean { return false; }
  recordedOutcome(): CommissioningResult | undefined { return undefined; }
  async settleForClose(): Promise<CommissioningResult | undefined> { await this.transfer.cancel(); return undefined; }
  async cancel(): Promise<CommissioningResult | undefined> { await this.transfer.cancel(); return undefined; }

  async rollback(): Promise<ParameterStatus> {
    if (this.consumed) throw new Error("Review recovery again before another rollback attempt.");
    await this.parameters.cancelOwner(this.checkpoint.parameterOperation);
    await this.transfer.cancel();
    let parameters = await this.parameters.status();
    assertParameterOwner(this.checkpoint, parameters);
    const current = await this.transfer.status();
    if (current.program_revision !== this.checkpoint.origin.program_revision || current.status === "pending") throw new Error("Controller transfer ownership remains unresolved.");
    const states = Object.fromEntries(this.checkpoint.binding.states.map(member => {
      const value = current.values[member.symbol];
      if (typeof value !== "number" || !Number.isFinite(value)) throw new Error(`Recovery state ${member.symbol} is unavailable.`);
      return [member.symbol, value];
    }));
    Object.assign(states, this.checkpoint.references);
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
      const recordingResult = await (await RuntimeRecordingSession.recover(this.loop, this.checkpoint.recording)).stop();
      this.checkpoint = { ...this.checkpoint, recordingResult };
      await this.writer(this.checkpoint);
    }
    this.checkpoint = { ...this.checkpoint, stage: "recovered" };
    await this.writer(this.checkpoint);
    // Consume only after every owned operation, recording stop, and durable
    // checkpoint write has completed. A failed recovery must remain retryable
    // from this reviewed instance without replaying any controller write.
    this.consumed = true;
    this.recoveryConfirmed = true;
    return parameters;
  }
}
function assertParameterOwner(checkpoint: CommissioningCheckpoint, current: ParameterStatus, allowOwnedPending = false): void {
  const unchanged = current.active_revision === checkpoint.origin.active_revision && sameParameterValues(current.values, checkpoint.origin.values);
  const adopted = current.previous_revision === checkpoint.origin.active_revision && !!current.previous_values
    && sameParameterValues(current.previous_values, checkpoint.origin.values) && sameParameterValues(current.values, checkpoint.candidate);
  if (current.status === "pending" && (!allowOwnedPending || current.transaction_id !== checkpoint.parameterOperation.id) || current.program_revision !== checkpoint.origin.program_revision
    || current.binding_revision !== checkpoint.origin.binding_revision || !unchanged && !adopted) throw new Error("Parameter ownership changed or remains pending; interrupted recovery cannot overwrite another operation.");
}
