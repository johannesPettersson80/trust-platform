import * as vscode from "vscode";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { EngineerStudy } from "./engineerStudyPersistence";
import type { StudioAnalysisSessions } from "./studioAnalysisSessions";
import { buildWorkflowResultSnapshot, type WorkflowResultSnapshotState } from "./workflowResultSnapshot";
import { workflowResultStorageKey } from "./panelPersistence";
import { saveWorkflowResultRevision } from "./workflowResult";
import { projectStudyStorage } from "./studyStore";
import type { ManualApplicationCloseRecovery } from "./engineerWorkflowSession";

const pending = new Map<string, Promise<void>>();

/** Closing retains evidence; reopening must obtain its own fresh runtime review. */
export function preserveUnresolvedRecovery(context: vscode.ExtensionContext, loop: ControlLoopDefinition,
  sessions: StudioAnalysisSessions, capture: (study: EngineerStudy) => WorkflowResultSnapshotState): boolean {
  const engineer = sessions.engineer;
  const study = engineer?.snapshotUnresolvedRecoveryStudy();
  if (!engineer || !study) return false;
  const ownerLoop = structuredClone(loop);
  const snapshot = structuredClone(capture(study));
  const original = snapshot.commissioning;
  const key = workflowResultStorageKey(ownerLoop);
  const settling = engineer.finishUnresolvedRecoveryForClose();
  sessions.dispose();
  let saving: Promise<void>;
  saving = (async () => {
    const outcome = await settling.catch(error => {
      if (!original) throw error;
      return { ...original, recoveryConfirmed: false, reason: `${original.reason} Close recovery unconfirmed: ${String(error)}` };
    });
    if (!outcome && !original) throw new Error("The campaign did not provide a terminal outcome; no result was invented.");
    const result = await buildWorkflowResultSnapshot(context, ownerLoop, { ...snapshot,
      commissioning: outcome ?? original, recording: outcome?.recording ?? snapshot.recording });
    await saveWorkflowResultRevision(projectStudyStorage(ownerLoop, "history"), key, result, 20);
  })().finally(() => { if (pending.get(key) === saving) pending.delete(key); });
  pending.set(key, saving);
  void saving.catch(error => vscode.window.showWarningMessage(`Could not retain recovery for ${ownerLoop.name}: ${String(error)}`));
  return true;
}

/** A standard Manual transaction has no serializable review token; retain only native recovery evidence until reopen. */
export function preservePendingManualApplication(context: vscode.ExtensionContext, loop: ControlLoopDefinition, sessions: StudioAnalysisSessions): boolean {
  const engineer = sessions.engineer;
  if (!engineer?.hasPendingManualApplication()) return false;
  const key = workflowResultStorageKey(loop);
  const settling = engineer.finishPendingManualApplicationForClose();
  sessions.dispose();
  const pendingResult = settling.then(() => undefined).finally(() => {
    if (pending.get(key) === pendingResult) pending.delete(key);
  });
  pending.set(key, pendingResult);
  return true;
}

export function deferRecoveryReopen(loop: ControlLoopDefinition, reopen: () => void): boolean {
  const saving = pending.get(workflowResultStorageKey(loop));
  if (!saving) return false;
  void vscode.window.withProgress({ location: vscode.ProgressLocation.Notification,
    title: `Retaining recovery for ${loop.name}` }, () => saving).then(reopen, () => undefined);
  return true;
}

/** A reopened panel must not adopt a recording while its original acquisition is restoring it. */
export function preservePendingIdentification(loop: ControlLoopDefinition, sessions: StudioAnalysisSessions): boolean {
  if (!sessions.engineer?.hasPendingIdentification()) return false;
  const key = workflowResultStorageKey(loop);
  const settling = sessions.engineer.finishIdentificationForClose();
  sessions.dispose();
  const saving = settling.finally(() => { if (pending.get(key) === saving) pending.delete(key); });
  pending.set(key, saving);
  void saving.catch(error => vscode.window.showWarningMessage(`Identification recovery remains pending: ${String(error)}`));
  return true;
}
