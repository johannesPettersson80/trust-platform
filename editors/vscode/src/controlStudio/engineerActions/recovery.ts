import { ObservationDetached, ObservationRequirementFailed, PermittedRangeLeft, resumeApplyObservation, type ObservedCheckpoint,
  type ResumedObservation } from "../applyObservation";
import type { CommissioningCheckpointWriter } from "../commissioningCheckpoint";
import type { CommissioningCriteria } from "../commissioningAssessment";
import { commissioningObservation, commissioningTarget } from "../commissioningObservation";
import { formatEngineeringNumber } from "../chartContract";
import { InterruptedCommissioningRecovery } from "../interruptedCommissioningRecovery";
import type { ControlLoopDefinition } from "../loopDefinition";
import type { LoopRecording } from "../loopMonitor";
import { sameParameterHistory, sameParameterValues, type ParameterApplicationClient } from "../parameterApplication";
import type { CommissioningResult } from "../proportionalCommissioning";
import { connectRuntimeParameters } from "../runtimeParameterConnection";
import { bindingRuntime, classifyMainWorkflowFailure, MainWorkflowError, nextActionSentence, presentMainWorkflowFailure,
  type MainWorkflowFailure } from "../mainWorkflowFailure";
import type { EngineerActionContext, EngineerActionHost, Respond } from "./host";
import { explanation } from "./values";
import { RuntimeTransferClient, type TransferStatus } from "../runtimeTransfer";
import { bindRuntimeConnection } from "../runtimeConnection";

/** Reviews a replacement runtime after an interrupted commissioning, before its operation is archived. */
export async function reviewReplacementRuntime(host: EngineerActionHost, { respond }: EngineerActionContext): Promise<boolean> {
  if (!host.state.interruptedCheckpoint) throw new Error("No interrupted commissioning checkpoint is retained.");
  const current = await (await bindingRuntime(() => connectRuntimeParameters(host.loop()))).client.status();
  if (current.program_revision === host.state.interruptedCheckpoint.origin.program_revision || current.status === "pending") {
    throw new MainWorkflowError("The original runtime is still active or has a pending operation. Review rollback to recover that operation.", { cause: "runtime", nextAction: "restore" });
  }
  host.state.runtimeReplacementReview = current;
  host.state.review = undefined;
  await respond({ status: "review", runtimeReplacementReview: true, replacementRuntimeAvailable: true, previous: host.state.interruptedCheckpoint.origin.values, next: current.values,
    reason: "A new runtime epoch is active. Confirm archiving the previous unresolved operation as historical and starting a fresh study. This writes no controller values and does not claim the previous operation completed." });
  return true;
}

/** Archives the interrupted operation as unresolved history once the replacement runtime was reviewed. */
export async function retireInterruptedOperation(host: EngineerActionHost, { post }: EngineerActionContext): Promise<boolean> {
  const reviewed = host.state.runtimeReplacementReview;
  host.state.runtimeReplacementReview = undefined;
  const current = await (await bindingRuntime(() => connectRuntimeParameters(host.loop()))).client.status();
  if (!sameParameterHistory(current, reviewed) || current.status === "pending" || !host.retainCommissioningCheckpoint) throw new Error("The replacement runtime changed; review it again.");
  await host.retainCommissioningCheckpoint({ ...host.state.interruptedCheckpoint!, stage: "retired", replacementProgramRevision: current.program_revision });
  host.state.interruptedCheckpoint = undefined; host.state.commissioning = undefined; host.state.commissioned = undefined;
  host.invalidate();
  await post({ status: "edited", recoveryResolved: true, reason: "Previous runtime operation retained as unresolved history. Find or supply a model and verify against the current runtime before application." });
  return true;
}

/** Reviews the rollback of an interrupted commissioning on its original runtime. */
export async function reviewInterruptedRollback(host: EngineerActionHost, { respond }: EngineerActionContext): Promise<boolean> {
  if (!host.retainCommissioningCheckpoint) throw new Error("The commissioning recovery journal is unavailable.");
  const connection = await bindingRuntime(() => connectRuntimeParameters(host.loop()));
  const current = await connection.client.status();
  if (current.program_revision !== host.state.interruptedCheckpoint!.origin.program_revision && current.status !== "pending") {
    const reason = "The runtime restarted. Review its current settings before continuing.";
    await respond({ status: "failed", replacementRuntimeAvailable: true, failure: presentMainWorkflowFailure(reason, { cause: "runtime", nextAction: "none" }), reason }); return true;
  }
  const recovery = await InterruptedCommissioningRecovery.review(host.loop(), host.state.interruptedCheckpoint!, connection.client,
    async checkpoint => { await host.retainCommissioningCheckpoint!(checkpoint); host.state.interruptedCheckpoint = checkpoint.stage === "recovered" ? undefined : checkpoint; });
  host.state.review = { client: connection.client, value: recovery.parameterReview(), rollback: true, commissioning: recovery };
  await respond({ status: "review", rollback: true, previous: host.state.review.value.observed.values,
    next: host.state.review.value.values, reason: recovery.rollbackDescription() });
  return true;
}

/**
 * Reattaches to the Apply observation the runtime is recording when the panel reopens (A8.5, A8.6, B6.3). It follows the recording to
 * its end and reports the assessed outcome. It starts no write, except the restore that a departure from the permitted range requires.
 */
export async function reattachApplyObservation(host: EngineerActionHost, checkpoint: ObservedCheckpoint, post: Respond,
  detached: () => boolean): Promise<void> {
  try {
    const reattach: Reattach = { host, checkpoint, criteria: observationCriteria(host, checkpoint), post, detached };
    await postWhileOpen(reattach, { status: "running", applicationAction: "apply", activity: "apply-observing",
      reason: "Reattaching to the response the runtime is recording." });
    const resumed = await resumeOutcome(reattach);
    if (resumed) await settlePass(reattach, resumed);
  } catch (error) {
    if (!(error instanceof ObservationDetached) && !detached()) await reportReattachFailure(post, error);
  }
}

/** One reattachment: the checkpoint it owns, the criteria it assesses against, and where it reports (A10.4). */
interface Reattach {
  readonly host: EngineerActionHost;
  readonly checkpoint: ObservedCheckpoint;
  readonly criteria: CommissioningCriteria;
  readonly post: Respond;
  readonly detached: () => boolean;
}

/** The requirements the response is assessed against: the study's model and requirements, as the settings were reviewed (B6.3). */
function observationCriteria(host: EngineerActionHost, checkpoint: ObservedCheckpoint): CommissioningCriteria {
  const loop = host.loop();
  const model = host.model();
  const task = host.state.task;
  if (!model || !task || model.revision !== checkpoint.model.revision) {
    throw new MainWorkflowError("The study's model or requirements no longer match the applied response, so the observation cannot be assessed.",
      { cause: "settings", nextAction: "restore" });
  }
  const { binding, candidate } = checkpoint;
  const onOff = loop.controller.onOff;
  return { target: commissioningTarget(loop, model, task, binding),
    durationSeconds: task.durationSeconds, settlingTimeSeconds: task.settlingTimeSeconds, maximumError: task.maximumError,
    taskPeriodMs: loop.controller.taskPeriodMs, processRange: { ...loop.signals.processVariable.range },
    outputLimits: { minimum: binding.output.minimum, maximum: binding.output.maximum },
    observed: commissioningObservation(loop, model, task, candidate),
    ...(onOff ? { switching: { low: onOff.lowOutput, high: onOff.highOutput, minimumOnSeconds: candidate[onOff.minimumOnTimeSymbol],
      minimumOffSeconds: candidate[onOff.minimumOffTimeSymbol] } } : {}) };
}

/** Follows the retained recording to its end. A failed requirement and a departure from the permitted range are reported here. */
async function resumeOutcome(r: Reattach): Promise<ResumedObservation | undefined> {
  try {
    return await resumeApplyObservation(r.host.loop(), r.checkpoint, r.criteria, {
      retain: recording => retainObservedRecording(r.host, recording),
      progress: text => postWhileOpen(r, { status: "running", applicationAction: "apply", activity: "apply-observing", reason: text }),
      detached: r.detached,
    });
  } catch (error) {
    if (error instanceof ObservationRequirementFailed && error.recording) await reportAppliedFailure(r, error.message, error.recording);
    else if (error instanceof PermittedRangeLeft) await restoreAfterDeparture(r, error);
    else throw error;
    return undefined;
  }
}

/** The study keeps the observed recording before the runtime's buffer is released (runtimeRecording.ts). */
async function retainObservedRecording(host: EngineerActionHost, recording: LoopRecording): Promise<void> {
  if (!host.retainRecording) throw new Error("The study cannot keep the observed recording, so the runtime keeps it until it can.");
  await host.retainRecording(recording);
}

/** The response passed: the settings stay applied, as an Apply leaves them, and Keep in project becomes available (B6.3). */
async function settlePass(r: Reattach, resumed: ResumedObservation): Promise<void> {
  const loop = r.host.loop();
  const parameters = await (await parameterClient(loop)).status();
  if (parameters.program_revision !== r.checkpoint.origin.program_revision || !sameParameterValues(parameters.values, r.checkpoint.candidate)) {
    await reportAppliedFailure(r, "Controller parameters changed during commissioning.", resumed.recording);
    return;
  }
  // Read before the journal records completion, so a failed read leaves the observation journalled, as a failed parameter read does.
  const transfers = await observedTransfers(r);
  const reason = `Auto commissioning passed after reopening: recorded target error ${formatEngineeringNumber(resumed.maximumError, 5)} ${loop.signals.processVariable.unit}.`;
  await retainCompleted(r, resumed.recording);
  const result = observationResult(r, { status: "passed", reason, recording: resumed.recording, maximumError: resumed.maximumError,
    parameters, recoveryConfirmed: true }, transfers);
  r.host.state.commissioned = result;
  await postWhileOpen(r, { status: "commissioned", applicationAction: "apply", commissioning: result, observed: parameters, reason });
}

/** A failed requirement, or a parameter record that changed: the settings stay applied, the response is stored, and Restore is offered (B6.3). */
async function reportAppliedFailure(r: Reattach, reason: string, recording: LoopRecording): Promise<void> {
  const failure: MainWorkflowFailure = { cause: "settings", nextAction: "restore" };
  await retainCompleted(r, recording);
  const result = observationResult(r, { status: "failed", reason, recording, failure, recoveryConfirmed: true });
  r.host.state.commissioned = result;
  await postWhileOpen(r, { status: "failed", applicationAction: "apply", applicationAvailability: "restore", commissioning: result,
    failure: presentMainWorkflowFailure(reason, failure), reason });
}

/** A departure from the permitted range restores the previous settings at once, then reports why (B6.3, B6.7). */
async function restoreAfterDeparture(r: Reattach, departure: PermittedRangeLeft): Promise<void> {
  const loop = r.host.loop();
  const checkpoint = departure.recording ? { ...r.checkpoint, recordingResult: departure.recording } : r.checkpoint;
  const recovery = await InterruptedCommissioningRecovery.review(loop, checkpoint, await parameterClient(loop), checkpointWriter(r.host));
  const parameters = await recovery.rollback();
  const reason = `${departure.statement}; the previous settings were restored, and ${recovery.modeStatement()}.`;
  const failure: MainWorkflowFailure = { cause: "model-or-data", nextAction: "none" };
  const result = observationResult(r, { status: "failed", reason, recording: departure.recording, failure, parameters, recoveryConfirmed: true });
  r.host.state.commissioned = result;
  await postWhileOpen(r, { status: "failed", applicationAction: "apply", commissioning: result, observed: parameters,
    failure: presentMainWorkflowFailure(reason, failure), reason });
}

/** The runtime's parameter client for the loop; reads are the only calls made through it until a reviewed restore (A8.6). */
async function parameterClient(loop: ControlLoopDefinition): Promise<ParameterApplicationClient> {
  return (await bindingRuntime(() => connectRuntimeParameters(loop))).client;
}

/** The commissioning journal's writer. The session keeps the checkpoint it writes until that checkpoint is recovered (commissioningCheckpoint.ts). */
function checkpointWriter(host: EngineerActionHost): CommissioningCheckpointWriter {
  const retain = host.retainCommissioningCheckpoint;
  if (!retain) throw new Error("The commissioning recovery journal is unavailable.");
  return async checkpoint => {
    await retain(checkpoint);
    host.state.interruptedCheckpoint = checkpoint.stage === "recovered" ? undefined : checkpoint;
  };
}

/** The journal keeps the checkpoint completed with its response: the settings stay applied until Restore or Keep in project (A8.6). */
async function retainCompleted(r: Reattach, recording: LoopRecording): Promise<void> {
  await checkpointWriter(r.host)({ ...r.checkpoint, stage: "completed", recordingResult: recording });
}

/**
 * The confirmed transfers of the Apply a reopened observation belongs to (B6.3, A8.6): those journalled before its target change, and
 * the target change itself as the runtime still reports it under its journalled identity. Nothing is invented, so a receipt without
 * that evidence cannot pass the saved-result check.
 */
async function observedTransfers(r: Reattach): Promise<TransferStatus[]> {
  const journalled = [...(r.checkpoint.observation?.transfers ?? [])];
  const owner = r.checkpoint.transfer;
  if (!owner) return journalled;
  const connection = await bindRuntimeConnection(r.host.loop().runtimeId);
  const status = await new RuntimeTransferClient(r.checkpoint.binding, r.checkpoint.origin.program_revision, connection.request).status();
  return status.transaction?.id === owner.id && !journalled.some(transfer => transfer.transaction?.id === owner.id)
    ? [...journalled, status] : journalled;
}

/** The commissioning receipt of a reattached observation, with the confirmed transfers it can show: by default the journalled ones. */
function observationResult(r: Reattach, fields: Pick<CommissioningResult, "status" | "reason" | "recording" | "maximumError" | "parameters"
  | "failure" | "recoveryConfirmed">, transfers: readonly TransferStatus[] = r.checkpoint.observation?.transfers ?? []): CommissioningResult {
  const loop = r.host.loop();
  const setpoint = loop.signals.setpoint!;
  return { ...fields, transfers: [...transfers], target: r.criteria.target, modelRevision: r.checkpoint.model.revision, criteria: r.criteria,
    channels: [setpoint, loop.signals.processVariable, loop.signals.manipulatedVariable, ...(loop.signals.additional ?? [])]
      .map(signal => ({ id: signal.symbol, unit: signal.unit })) };
}

/** A failure no typed outcome covers keeps the journalled operation for Restore, and says why (A11.2). */
async function reportReattachFailure(post: Respond, error: unknown): Promise<void> {
  const failure = classifyMainWorkflowFailure(error);
  const reason = explanation(error);
  await post({ status: "failed", applicationAction: "apply", commissioningCheckpoint: true, applicationAvailability: "restore",
    failure: presentMainWorkflowFailure(reason, failure), reason: `${reason}${nextActionSentence(failure.nextAction)}` });
}

/** Posts while the panel is open: a detached observation reports nothing, since closing released its client only (A10.4). */
async function postWhileOpen(r: Reattach, payload: Record<string, unknown>): Promise<void> {
  if (!r.detached()) await r.post(payload);
}
