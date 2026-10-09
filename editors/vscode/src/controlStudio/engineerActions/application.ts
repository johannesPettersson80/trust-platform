import { controllerAdapter } from "../controllerAdapters";
import type { PendingManualApplicationOwner } from "../engineerWorkflowSession";
import { matchesGainRecovery } from "../gainRecoveryContext";
import { InterruptedCommissioningRecovery } from "../interruptedCommissioningRecovery";
import { dispatchPendingManualApplication } from "../panelPersistence";
import { sameParameterHistory, type ParameterStatus } from "../parameterApplication";
import { PidCommissioning } from "../pidCommissioning";
import { ProportionalCommissioning } from "../proportionalCommissioning";
import { connectRuntimeParameters } from "../runtimeParameterConnection";
import { requireLiveWrite } from "../runtimeTarget";
import { restoreUnavailableReason } from "../restoreAvailability";
import { bindingRuntime, MainWorkflowError, presentMainWorkflowFailure } from "../mainWorkflowFailure";
import type { EngineerActionContext, EngineerActionHost } from "./host";
import { explanation } from "./values";
import { measuredProcessNoise } from "../processNoiseEvidence";

/** Confirms the reviewed application or rollback: commissioning in Auto, or the Manual transaction. */
export async function confirmReviewedOperation(host: EngineerActionHost, { respond, generation }: EngineerActionContext): Promise<boolean> {
  const retained = host.state.review; host.state.review = undefined;
  if (!retained) throw new Error("Review the exact application or rollback first.");
  if (!retained.rollback) await host.checkVerifiedProject();
  if (generation !== host.state.generation) return true;
  host.state.busy = "apply";
  try {
    if (retained.commissioning && !retained.rollback) {
      if (!host.retainCommissioningCheckpoint) throw new Error("Persist commissioning recovery before dispatching the reviewed operation.");
      retained.commissioning.setCheckpointWriter(async checkpoint => {
        await host.retainCommissioningCheckpoint!(checkpoint);
        host.state.interruptedCheckpoint = checkpoint.stage === "recovered" ? undefined : checkpoint;
      });
      host.state.commissioning = retained.commissioning;
      await respond({ status: "commissioning", reason: "Commissioning the reviewed controller and recording its response. Cancel remains available." });
      const result = await retained.commissioning.run(); host.state.commissioned = result;
      if (result.recording) await host.retainRecording?.(result.recording);
      // The runtime's buffer is released only once the samples are stored in the study (runtimeRecording.ts).
      if (result.recording && host.retainRecording) await retained.commissioning.releaseRecording();
      // A typed failure keeps its own reason (B6.7); an untyped failure keeps the commissioning prefix as before.
      const commissioningReason = result.status === "passed" || result.failure ? result.reason : `Auto commissioning ${result.status}: ${result.reason}`;
      await respond({ status: result.status === "passed" ? "commissioned" : "failed", commissioning: result,
        observed: result.parameters, initialization: host.state.initialization?.evaluation,
        ...(result.status !== "passed" && (result.failure || result.recoveryConfirmed)
          ? { failure: presentMainWorkflowFailure(commissioningReason, result.failure ?? { cause: "model-or-data", nextAction: "identify" }) } : {}),
        reason: commissioningReason });
      return true;
    }
    if (retained.rollback && retained.commissioning) {
      host.state.commissioning = retained.commissioning;
      await respond({ status: "commissioning", reason: "Recovering the reviewed controller. Cancel can retry an unconfirmed recovery." });
    }
    if (!retained.commissioning) {
      const owner: PendingManualApplicationOwner = {
        operation: retained.rollback ? "rollback" : "apply",
        origin: { program_revision: retained.value.observed.program_revision, binding_revision: retained.value.observed.binding_revision,
          review: { transaction_id: retained.value.transaction_id, active_revision: retained.value.observed.active_revision, previous_revision: retained.value.observed.previous_revision,
            values: { ...retained.value.observed.values }, requested: { ...retained.value.values } } },
      };
      let ownerRetained = false;
      const applying = dispatchPendingManualApplication(owner, async value => {
        if (!host.retainPendingManualApplication) throw new Error("Could not retain the pending Manual transaction before dispatch.");
        await host.retainPendingManualApplication(value);
        ownerRetained = true;
      }, async (): Promise<ParameterStatus> => {
        const observed = retained.rollback ? await retained.client.rollback(retained.value, false) : await retained.client.apply(retained.value, false);
        await respond({ status: "applied", observed, reason: retained.rollback
          ? "Previous values restored and observed after ST execution in Manual."
          : "Candidate values staged in Manual and observed after ST execution. Automatic-mode transfer remains a separate commissioning operation." });
        return observed;
      });
      const pendingApplication = {
        ...owner,
        settling: applying,
      };
      host.state.pendingManualApplication = pendingApplication;
      try { await applying; }
      catch (error) {
        if (!ownerRetained) throw error;
        host.state.busy = undefined;
        const reason = host.requireManualRecovery(owner,
          `The Manual ${owner.operation} could not be confirmed: ${explanation(error)}. Retry recovery to inspect the retained transaction.`);
        await respond({ status: "recovery-required", retryManualRecovery: true, reason });
      } finally { if (host.state.pendingManualApplication === pendingApplication) host.state.pendingManualApplication = undefined; }
      return true;
    }
    const observed = retained.rollback && retained.commissioning ? await retained.commissioning.rollback()
      : await retained.client.apply(retained.value, false);
    if (retained.rollback && retained.commissioning) host.state.commissioned = retained.commissioning.recordedOutcome();
    // A6.2 and B6.4: the restore names the mode it left the loop in, the one it was found in when that is known.
    const restoredMode = retained.commissioning instanceof InterruptedCommissioningRecovery ? `; ${retained.commissioning.modeStatement()}` : "";
    await respond({ status: "applied", observed, recoveryResolved: retained.rollback, applicationAction: retained.rollback ? "restore" : "apply", reason: retained.rollback ? `Previous values restored and observed after ST execution${restoredMode}.` : "Candidate values staged in Manual and observed after ST execution. Automatic-mode transfer remains a separate commissioning operation." });
  } finally { if (host.state.commissioning?.recoveryConfirmed !== false || host.state.commissioning instanceof InterruptedCommissioningRecovery) host.state.busy = undefined; }
  return true;
}

/** Apply and Restore: reviews the exact change, waiting for the validated operating point before an Auto commissioning. */
export async function reviewApplication(host: EngineerActionHost, { respond, generation, applicationMode }: EngineerActionContext, action: "apply" | "rollback"): Promise<boolean> {
  if (action === "apply") await host.checkVerifiedProject();
  const connection = await bindingRuntime(() => connectRuntimeParameters(host.loop()));
  requireLiveWrite(connection, "parameter application");
  if (action === "apply") {
    await host.checkVerifiedProject();
    const value = await connection.client.review(host.state.values!);
    await host.checkVerifiedProject();
    if (generation !== host.state.generation) return true;
    host.state.review = { client: connection.client, value, rollback: false };
    host.state.manualApplicationRecovery = undefined;
    if (applicationMode === "auto") {
      if (host.state.initialization?.evaluation.status !== "Pass" || host.state.initialization.projectRevision !== host.state.verified?.projectRevision) {
        throw new MainWorkflowError("Verify the Auto initialization case before commissioning.", { cause: "settings", nextAction: "validate" });
      }
      const settling = host.state.applySettling = { cancelled: false };
      let shown = "";
      try {
        host.state.review.commissioning = await controllerAdapter(host.loop()).commission(host.model()!, host.state.task!, connection.client, value, {
          // Only a changed reading is posted again.
          progress: async reason => { if (reason !== shown) { shown = reason; await respond({ status: "running", activity: "apply-settling", reason }); } },
          cancelled: () => settling.cancelled || generation !== host.state.generation },
          measuredProcessNoise(host.loop(), host.model(), host.state.campaigns));
      } finally { if (host.state.applySettling === settling) host.state.applySettling = undefined; }
    }
  } else {
    const observed = await connection.client.status();
    // B6.4: Restore is unavailable once the settings Apply wrote are no longer the active ones, so nothing is overwritten.
    const unavailable = restoreUnavailableReason(observed, host.state.commissioned?.parameters);
    if (unavailable) throw new MainWorkflowError(unavailable, { cause: "settings", nextAction: "none" });
    const matchesCommissioning = sameParameterHistory(observed, host.state.commissioned?.parameters)
      || (!host.state.commissioned?.parameters && host.state.commissioned?.recoveryConfirmed === false
        && matchesGainRecovery(observed, host.state.commissioned.gainRecovery));
    if (matchesCommissioning && host.state.commissioned) {
      controllerAdapter(host.loop()).assertRecoveryNeeded(host.state.commissioned, host.model());
    }
    const value = await connection.client.reviewRollback();
    if (generation !== host.state.generation) return true;
    let commissioning: ProportionalCommissioning | PidCommissioning | InterruptedCommissioningRecovery | undefined;
    if (matchesCommissioning && host.state.commissioned) {
      const model = host.model();
      if (!model || !host.state.task) throw new MainWorkflowError("Restore the compatible model and task before reviewing commissioning recovery.", { cause: "settings", nextAction: !model ? "identify" : "open-requirements" });
      commissioning = host.state.commissioning ?? await controllerAdapter(host.loop()).recover(
        model, host.state.task, connection.client, value, host.state.commissioned);
    }
    if (generation !== host.state.generation) return true;
    host.state.review = { client: connection.client, value, rollback: true, commissioning };
    host.state.manualApplicationRecovery = undefined;
  }
  await respond({ status: "review", rollback: host.state.review.rollback, previous: host.state.review.value.observed.values,
    next: host.state.review.value.values, reason: host.state.review.commissioning && !host.state.review.rollback
      ? host.state.review.commissioning.description() : host.state.review.commissioning
      ? host.state.review.commissioning.rollbackDescription() : "Review the exact change, then confirm application." });
  return true;
}
