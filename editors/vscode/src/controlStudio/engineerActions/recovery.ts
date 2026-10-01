import { InterruptedCommissioningRecovery } from "../interruptedCommissioningRecovery";
import { sameParameterHistory } from "../parameterApplication";
import { connectRuntimeParameters } from "../runtimeParameterConnection";
import type { EngineerActionContext, EngineerActionHost } from "./host";

/** Reviews a replacement runtime after an interrupted commissioning, before its operation is archived. */
export async function reviewReplacementRuntime(host: EngineerActionHost, { respond }: EngineerActionContext): Promise<boolean> {
  if (!host.state.interruptedCheckpoint) throw new Error("No interrupted commissioning checkpoint is retained.");
  const current = await (await connectRuntimeParameters(host.loop())).client.status();
  if (current.program_revision === host.state.interruptedCheckpoint.origin.program_revision || current.status === "pending") {
    throw new Error("The original runtime is still active or has a pending operation. Review rollback to recover that operation.");
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
  const current = await (await connectRuntimeParameters(host.loop())).client.status();
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
  const connection = await connectRuntimeParameters(host.loop());
  const current = await connection.client.status();
  if (current.program_revision !== host.state.interruptedCheckpoint!.origin.program_revision && current.status !== "pending") {
    await respond({ status: "failed", replacementRuntimeAvailable: true, reason: "The runtime restarted. Review its current settings before continuing." }); return true;
  }
  const recovery = await InterruptedCommissioningRecovery.review(host.loop(), host.state.interruptedCheckpoint!, connection.client,
    async checkpoint => { await host.retainCommissioningCheckpoint!(checkpoint); host.state.interruptedCheckpoint = checkpoint.stage === "recovered" ? undefined : checkpoint; });
  host.state.review = { client: connection.client, value: recovery.parameterReview(), rollback: true, commissioning: recovery };
  await respond({ status: "review", rollback: true, previous: host.state.review.value.observed.values,
    next: host.state.review.value.values, reason: recovery.rollbackDescription() });
  return true;
}
