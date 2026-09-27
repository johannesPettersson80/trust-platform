import { connectRuntimeParameters } from "./runtimeParameterConnection";
import { sameParameterValues, type ParameterStatus } from "./parameterApplication";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { PendingManualApplicationOwner, ManualApplicationCloseRecovery } from "./engineerWorkflowSession";

/** Inspect a retained transaction; never dispatch a replacement write. */
export async function reconcileManualApplication(loop: ControlLoopDefinition,
  recovery: PendingManualApplicationOwner | ManualApplicationCloseRecovery,
  publish: (payload: Record<string, unknown>) => Promise<void>,
  markUnresolved: (owner: PendingManualApplicationOwner, reason: string) => string,
  clearOwner: () => void): Promise<boolean> {
    const post = (payload: Record<string, unknown>) => publish({ ...payload,
      applicationAction: recovery.operation === "rollback" ? "restore" : "apply",
      ...(payload.status === "recovery-required" ? { retryManualRecovery: true } : { recoveryResolved: true }) });
    let observed: ParameterStatus;
    try {
      observed = await (await connectRuntimeParameters(loop)).client.status();
    } catch (error) {
      const reason = markUnresolved(recovery, `Could not inspect the retained ${recovery.operation}: ${explanation(error)} Make a fresh review after reconnecting.`);
      await post({ status: "recovery-required", reason });
      return false;
    }
    if (observed.program_revision !== recovery.origin.program_revision || observed.binding_revision !== recovery.origin.binding_revision) {
      const reason = markUnresolved(recovery, `The program or parameter binding changed since the operation was issued. The ${recovery.operation} result is unresolved; make a fresh review.`);
      await post({ status: "recovery-required", observed, reason });
      return false;
    }
    const operationId = recovery.origin.review?.transaction_id;
    if (operationId && (observed.status === "pending" || observed.transaction_id !== operationId)) {
      try {
        const client = (await connectRuntimeParameters(loop)).client;
        const cancelled = await client.cancelOwner({ id: operationId, program: recovery.origin.program_revision, binding: recovery.origin.binding_revision });
        if (cancelled === "before-dispatch") {
          clearOwner();
          await post({ status: "failed", observed, reason: `The prepared ${recovery.operation} was cancelled before dispatch. No operation was replayed; make a fresh review to apply settings.` });
          return true;
        }
        observed = await client.status();
      } catch (error) {
        const reason = markUnresolved(recovery, `The retained parameter operation could not be reconciled: ${explanation(error)}`);
        await post({ status: "recovery-required", observed, reason });
        return false;
      }
    }
    if (observed.status === "pending" || observed.status === "idle") {
      const reason = markUnresolved(recovery, `The current runtime did not provide a terminal ${recovery.operation} status. Make a fresh review before dependent work.`);
      await post({ status: "recovery-required", observed, reason });
      return false;
    }
    if ("observed" in recovery && (observed.status !== recovery.observed.status || observed.active_revision !== recovery.observed.active_revision
      || !sameParameterValues(observed.values, recovery.observed.values))) {
      const reason = markUnresolved(recovery, `The current runtime status changed after the retained ${recovery.operation}. Make a fresh review before dependent work.`);
      await post({ status: "recovery-required", observed, reason });
      return false;
    }
    const frozen = recovery.origin.review;
    const matches = frozen && (!frozen.transaction_id || observed.transaction_id === frozen.transaction_id) && (observed.status === "applied"
      ? sameParameterValues(observed.values, frozen.requested) && (recovery.operation === "rollback"
        ? observed.active_revision === frozen.previous_revision
        : observed.previous_revision === frozen.active_revision && !!observed.previous_values && sameParameterValues(observed.previous_values, frozen.values))
      : observed.status === "rejected" && observed.active_revision === frozen.active_revision && sameParameterValues(observed.values, frozen.values));
    if (!matches) {
      const reason = markUnresolved(recovery, "The runtime result does not match the exact parameter record retained before dispatch. Review current and requested values before further application; the operation was not repeated.");
      await post({ status: "recovery-required", observed, reason });
      return false;
    }
    clearOwner();
    if (observed.status === "applied") {
      await post({ status: "applied", observed,
        reason: `Retained ${recovery.operation} is confirmed by the current runtime status.` });
    } else {
      await post({ status: "failed", observed,
        reason: `Retained ${recovery.operation} was rejected by the current runtime: ${observed.reason ?? ("reason" in recovery ? recovery.reason : "no reason supplied")}` });
    }
    return true;
}
function explanation(error: unknown): string { return error instanceof Error ? error.message : String(error); }
