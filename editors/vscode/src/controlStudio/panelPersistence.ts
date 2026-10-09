import { decodeCommissioningCheckpoint, type CommissioningCheckpoint } from "./commissioningCheckpoint";
import { decodeVerificationAttempt } from "./engineerStudyPersistence";
import type { RuntimeVerificationAttempt } from "./verificationClient";
import type * as vscode from "vscode";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { PlantControlStudioHandoff } from "./handoff";
import type { ExcludedInterval } from "./identification";
import { decodeWorkflowResultHistoryState, type ControlStudioWorkflowResult } from "./workflowResult";
import { decodeControlExperimentRecovery, type ControlExperimentPlan, type ControlExperimentRecovery } from "./experiment";
import type { RecordingRecoveryOwner } from "./runtimeRecording";
import type { PendingManualApplicationOwner } from "./engineerWorkflowSession";
import type { MimoAnalysisSnapshot } from "./mimoAnalysisSession";
import { projectStudyState, updateProjectStudy } from "./studyStore";

/** Reuses workspace persistence for the one currently owned isolated operation. */
const isolatedOwnerWrites = new Map<string, Promise<void>>();
export async function storeIsolatedOperation(context: vscode.ExtensionContext, loop: ControlLoopDefinition,
  attempt: RuntimeVerificationAttempt): Promise<void> {
  if (JSON.stringify(attempt).length > 16 * 1024 * 1024 || !decodeVerificationAttempt(attempt)) throw new Error("Isolated-operation recovery record is invalid or exceeds its storage bound.");
  const key = isolatedOperationKey(loop), copied = structuredClone(attempt);
  const writing = (isolatedOwnerWrites.get(key) ?? Promise.resolve()).catch(() => undefined).then(async () => {
    const previous = restoreIsolatedOperation(context, loop);
    if (previous && previous.jobId !== copied.jobId) {
      // A late terminal callback from a closed panel must not replace a newer owner.
      if (copied.status !== "cancellation-unconfirmed") return;
      if (previous.status === "cancellation-unconfirmed") throw new Error("Resolve the retained isolated operation before dispatching another one.");
    }
    await context.workspaceState.update(key, { loopRevision: loop.revision, attempt: copied });
  });
  isolatedOwnerWrites.set(key, writing);
  try { await writing; } finally { if (isolatedOwnerWrites.get(key) === writing) isolatedOwnerWrites.delete(key); }
}
export function restoreIsolatedOperation(context: vscode.ExtensionContext, loop: ControlLoopDefinition): RuntimeVerificationAttempt | undefined {
  const saved: unknown = context.workspaceState.get(isolatedOperationKey(loop));
  if (saved === undefined) return undefined;
  if (!record(saved) || saved.loopRevision !== loop.revision) throw new Error("An isolated-operation recovery record belongs to a different loop revision. Reopen that loop to reconcile its runtime operation.");
  const attempt = decodeVerificationAttempt(saved.attempt);
  if (!attempt) throw new Error("The retained isolated-operation identity cannot be decoded; further runtime work is blocked.");
  return attempt;
}
function isolatedOperationKey(loop: ControlLoopDefinition): string {
  return `trust.controlStudio.isolated-operation.${loop.projectId}.${loop.runtimeId}.${loop.id}`;
}

const commissioningWrites = new Map<string, Promise<void>>();
function commissioningKey(loop: ControlLoopDefinition): string { return `trust.controlStudio.commissioning.${loop.projectId}.${loop.runtimeId}.${loop.id}`; }
export function restoreCommissioningCheckpoint(context: vscode.ExtensionContext, loop: ControlLoopDefinition): CommissioningCheckpoint | undefined {
  const value: unknown = context.workspaceState.get(commissioningKey(loop));
  if (value === undefined) return undefined;
  const checkpoint = decodeCommissioningCheckpoint(value, loop);
  if (!checkpoint) throw new Error("The retained commissioning checkpoint does not match this loop. Reopen the original loop to review recovery.");
  return checkpoint;
}
export async function storeCommissioningCheckpoint(context: vscode.ExtensionContext, loop: ControlLoopDefinition, input: CommissioningCheckpoint): Promise<void> {
  const checkpoint = decodeCommissioningCheckpoint(input, loop);
  if (!checkpoint) throw new Error("Commissioning recovery could not be retained; no further operation may be dispatched.");
  const key = commissioningKey(loop);
  const writing = (commissioningWrites.get(key) ?? Promise.resolve()).catch(() => undefined).then(async () => {
    const previous = restoreCommissioningCheckpoint(context, loop);
    if (previous?.id === checkpoint.id && ["recovered", "retired"].includes(previous.stage)
      && checkpoint.stage !== previous.stage) return;
    if (previous && previous.id !== checkpoint.id) {
      if (checkpoint.stage !== "prepared") return;
      if (previous.stage !== "completed" && previous.stage !== "recovered" && previous.stage !== "retired") throw new Error("Recover the previous commissioning operation before starting another.");
    }
    if (previous && previous.id !== checkpoint.id) {
      const history = context.workspaceState.get<CommissioningCheckpoint[]>(`${key}.history`, []);
      await context.workspaceState.update(`${key}.history`, [...history, previous].slice(-20));
    }
    await context.workspaceState.update(key, checkpoint);
  });
  commissioningWrites.set(key, writing);
  try { await writing; } finally { if (commissioningWrites.get(key) === writing) commissioningWrites.delete(key); }
}

export function selectedIntervals(endSeconds: number, excluded: readonly ExcludedInterval[]): Array<{ startSeconds: number; endSeconds: number }> {
  if (endSeconds <= 0) return [];
  const ordered = excluded.filter((item) => item.endSeconds >= 0 && item.startSeconds <= endSeconds)
    .map((item) => ({ startSeconds: Math.max(0, item.startSeconds), endSeconds: Math.min(endSeconds, item.endSeconds) }))
    .sort((left, right) => left.startSeconds - right.startSeconds);
  const selected: Array<{ startSeconds: number; endSeconds: number }> = [];
  let cursor = 0;
  for (const item of ordered) {
    if (item.startSeconds > cursor) selected.push({ startSeconds: cursor, endSeconds: item.startSeconds });
    cursor = Math.max(cursor, item.endSeconds);
  }
  if (cursor < endSeconds) selected.push({ startSeconds: cursor, endSeconds });
  return selected;
}

export function restoreWorkflowResultHistory(
  loop: ControlLoopDefinition | undefined
): { history: ControlStudioWorkflowResult[]; unavailableReason?: string } {
  if (!loop) return { history: [] };
  const decoded = decodeWorkflowResultHistoryState(projectStudyState(loop).history);
  return {
    ...decoded,
    history: decoded.history.filter((result) => result.projectId === loop.projectId && result.loop.id === loop.id),
  };
}

export function workflowResultStorageKey(loop: ControlLoopDefinition): string {
  return `trust.controlStudio.result.${loop.projectId}.${loop.id}`;
}

export function restoreExperimentRecovery(
  context: vscode.ExtensionContext,
  loop: ControlLoopDefinition | undefined
): Pick<ControlExperimentRecovery, "experimentId" | "plan" | "recordingOwner"> | undefined {
  if (!loop) return undefined;
  return decodeControlExperimentRecovery(
    context.workspaceState.get(experimentRecoveryStorageKey(loop)),
    { projectId: loop.projectId, loopId: loop.id, loopRevision: loop.revision }
  );
}

export async function storeExperimentRecovery(context: vscode.ExtensionContext | undefined, loop: ControlLoopDefinition,
  experiment: string | ControlExperimentPlan, recordingOwner?: RecordingRecoveryOwner): Promise<void> {
  const value = typeof experiment === "string" ? {
    schemaVersion: 1, experimentId: experiment, projectId: loop.projectId, loopId: loop.id, loopRevision: loop.revision,
  } : recordingOwner ? {
    schemaVersion: 2, experimentId: experiment.id, projectId: loop.projectId, loopId: loop.id, loopRevision: loop.revision,
    plan: experiment, recordingOwner,
  } : undefined;
  const recovery = value && decodeControlExperimentRecovery(value, {
    projectId: loop.projectId, loopId: loop.id, loopRevision: loop.revision,
  });
  if (!recovery) throw new Error("Test experiment recovery is invalid or outside the active loop scope.");
  await context?.workspaceState.update(experimentRecoveryStorageKey(loop), structuredClone(value));
}

export async function clearExperimentRecovery(context: vscode.ExtensionContext | undefined, loop: ControlLoopDefinition): Promise<void> {
  await context?.workspaceState.update(experimentRecoveryStorageKey(loop), undefined);
}

export function experimentRecoveryStorageKey(loop: ControlLoopDefinition): string {
  return `trust.controlStudio.experiment.${loop.projectId}.${loop.id}`;
}

/** MIMO analysis is an analysis of the loop's study, so it is kept in the project study (A8.1). */
export async function storeMimoAnalysis(loop: ControlLoopDefinition, snapshot: MimoAnalysisSnapshot): Promise<void> {
  if (JSON.stringify(snapshot).length > 4 * 1024 * 1024 || !record(snapshot) || !record(snapshot.result)) throw new Error("MIMO analysis evidence is invalid or exceeds its storage bound.");
  await updateProjectStudy(loop, "mimo", { loopRevision: loop.revision, snapshot: structuredClone(snapshot) });
}

export function restoreMimoAnalysis(loop: ControlLoopDefinition): MimoAnalysisSnapshot | undefined {
  const value: unknown = projectStudyState(loop).mimo;
  if (value === undefined) return undefined;
  if (!record(value) || value.loopRevision !== loop.revision || !record(value.snapshot)) return undefined;
  return value.snapshot as unknown as MimoAnalysisSnapshot;
}

export function mimoAnalysisStorageKey(loop: ControlLoopDefinition): string {
  return `trust.controlStudio.mimo-analysis.${loop.projectId}.${loop.id}`;
}

/** A host-lifetime Manual owner is written before acknowledgement and has no review capability or client. */
export async function storePendingManualApplicationRecovery(context: vscode.ExtensionContext | { workspaceState: { update(key: string, value: unknown): PromiseLike<void> } },
  loop: ControlLoopDefinition, recovery: PendingManualApplicationOwner): Promise<void> {
  const owner = manualApplicationRecoveryOwner(loop, recovery);
  if (!owner) throw new Error("Manual recovery owner is invalid or outside the active loop scope.");
  await context.workspaceState.update(manualApplicationRecoveryStorageKey(loop), owner);
}

/** A native Manual transaction may begin only after its bounded host owner is durable. */
export async function dispatchPendingManualApplication<T>(owner: PendingManualApplicationOwner,
  store: (owner: PendingManualApplicationOwner) => Promise<void>, dispatch: () => Promise<T>): Promise<T> {
  await store(owner);
  return dispatch();
}

export function restoreManualApplicationRecovery(context: vscode.ExtensionContext | { workspaceState: { get(key: string): unknown } },
  loop: ControlLoopDefinition): PendingManualApplicationOwner | undefined {
  const owner = decodeManualApplicationRecoveryOwner(context.workspaceState.get(manualApplicationRecoveryStorageKey(loop)), loop);
  if (!owner) return undefined;
  return { operation: owner.operation, origin: owner.origin };
}

export async function clearManualApplicationRecovery(context: vscode.ExtensionContext | { workspaceState: { update(key: string, value: unknown): PromiseLike<void> } },
  loop: ControlLoopDefinition): Promise<void> {
  await context.workspaceState.update(manualApplicationRecoveryStorageKey(loop), undefined);
}

/** Calls the fresh status check once and forgets the owner only after its terminal result. */
export async function reconcileManualApplicationRecovery(context: vscode.ExtensionContext | {
  workspaceState: { get(key: string): unknown; update(key: string, value: unknown): PromiseLike<void> };
}, loop: ControlLoopDefinition, reconcile: (recovery: PendingManualApplicationOwner) => Promise<boolean>): Promise<boolean> {
  const recovery = restoreManualApplicationRecovery(context, loop);
  if (!recovery) return false;
  const terminal = await reconcile(recovery);
  if (terminal) await clearManualApplicationRecovery(context, loop);
  return terminal;
}

export function manualApplicationRecoveryStorageKey(loop: ControlLoopDefinition): string {
  return `trust.controlStudio.manual-application.${loop.projectId}.${loop.runtimeId}.${loop.id}`;
}

type ManualApplicationRecoveryOwner = {
  schemaVersion: 1;
  scope: { projectId: string; runtimeId: string; loopId: string; loopRevision: string };
  operation: "apply" | "rollback";
  origin: PendingManualApplicationOwner["origin"];
};

function manualApplicationRecoveryOwner(loop: ControlLoopDefinition, recovery: PendingManualApplicationOwner): ManualApplicationRecoveryOwner | undefined {
  const owner: unknown = {
    schemaVersion: 1,
    scope: { projectId: loop.projectId, runtimeId: loop.runtimeId, loopId: loop.id, loopRevision: loop.revision },
    operation: recovery.operation,
    origin: recovery.origin,
  };
  return decodeManualApplicationRecoveryOwner(owner, loop);
}

function decodeManualApplicationRecoveryOwner(value: unknown, loop: ControlLoopDefinition): ManualApplicationRecoveryOwner | undefined {
  if (!record(value) || value.schemaVersion !== 1 || !record(value.scope) || value.scope.projectId !== loop.projectId ||
      value.scope.runtimeId !== loop.runtimeId || value.scope.loopId !== loop.id || value.scope.loopRevision !== loop.revision ||
      (value.operation !== "apply" && value.operation !== "rollback") || !record(value.origin) || !text(value.origin.program_revision) ||
      !text(value.origin.binding_revision) || Object.keys(value).sort().join(",") !== "operation,origin,schemaVersion,scope") return undefined;
  const review = value.origin.review;
  if (review !== undefined) {
    const validValues = (values: unknown) => record(values) && !!loop.parameterBinding
      && Object.keys(values).length === loop.parameterBinding.parameters.length
      && loop.parameterBinding.parameters.every(parameter => typeof values[parameter.symbol] === "number"
        && Number.isFinite(values[parameter.symbol]) && (values[parameter.symbol] as number) >= parameter.minimum
        && (values[parameter.symbol] as number) <= parameter.maximum
        && (parameter.iec_type !== "REAL" || Math.fround(values[parameter.symbol] as number) === values[parameter.symbol]));
    if (!record(review) || review.transaction_id !== undefined && !text(review.transaction_id) || !text(review.active_revision) || !(review.previous_revision === null || text(review.previous_revision))
      || !validValues(review.values) || !validValues(review.requested)) return undefined;
  }
  return structuredClone(value) as unknown as ManualApplicationRecoveryOwner;
}

function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function text(value: unknown): value is string { return typeof value === "string" && value.trim().length > 0; }

export function controlStudioScopeKey(loop: ControlLoopDefinition | undefined, handoff: PlantControlStudioHandoff | undefined): string {
  if (loop) return `loop:${loop.projectId}:${loop.runtimeId}:${loop.id}`;
  if (handoff) return `plant:${handoff.projectId}:${handoff.runtimeId}:${handoff.plantId}`;
  return "unbound";
}
